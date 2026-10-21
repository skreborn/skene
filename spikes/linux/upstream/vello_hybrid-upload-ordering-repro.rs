//! vello_hybrid 0.2.0: an image uploaded after `destroy_image` (or after an upload that grows
//! the atlas array) in the same command encoder is overwritten when the encoder is submitted.
//! `upload_image` writes pixels with `queue.write_texture`, which runs before every command of
//! the next submitted encoder, while `destroy_image`'s clear and the atlas array's growth copy are
//! recorded into the caller's encoder, so they run after the write.
use vello_common::{
  kurbo::{Affine, Rect},
  paint::{Image, ImageId, ImageSource},
  peniko::{ImageQuality, ImageSampler, color::PremulRgba8},
  pixmap::Pixmap,
};
use vello_hybrid::{RenderSize, RenderTargetConfig, Renderer, Resources, Scene, TextureBindings};

const SIZE: u32 = 64;

fn solid(side: u16, [r, g, b, a]: [u8; 4]) -> Pixmap {
  Pixmap::from_parts(vec![PremulRgba8 { r, g, b, a }; usize::from(side) * usize::from(side)], side, side)
}

/// Draws `id` (side `side`) at the top left of a 64x64 frame with `encoder`, submits it, and
/// returns the frame's pixel at (2, 2).
fn draw_and_read(
  device: &wgpu::Device,
  queue: &wgpu::Queue,
  renderer: &mut Renderer,
  resources: &mut Resources,
  mut encoder: wgpu::CommandEncoder,
  id: ImageId,
  side: u16,
) -> [u8; 4] {
  let mut scene = Scene::new(SIZE as u16, SIZE as u16);
  scene.set_transform(Affine::IDENTITY);
  scene.set_paint(Image {
    image: ImageSource::opaque_id(id),
    sampler: ImageSampler { quality: ImageQuality::Low, ..ImageSampler::default() },
  });
  scene.fill_rect(&Rect::new(0., 0., f64::from(side), f64::from(side)));
  let target = device.create_texture(&wgpu::TextureDescriptor {
    label: None,
    size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: wgpu::TextureFormat::Rgba8Unorm,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    view_formats: &[],
  });
  let view = target.create_view(&wgpu::TextureViewDescriptor::default());
  renderer
    .render(&scene, resources, device, queue, &mut encoder, &RenderSize { width: SIZE, height: SIZE }, &view, &TextureBindings::new())
    .expect("render");
  let row = (SIZE * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
  let buffer = device.create_buffer(&wgpu::BufferDescriptor {
    label: None,
    size: u64::from(row * SIZE),
    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
  });
  encoder.copy_texture_to_buffer(
    target.as_image_copy(),
    wgpu::TexelCopyBufferInfo {
      buffer: &buffer,
      layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None },
    },
    wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
  );
  queue.submit([encoder.finish()]);
  buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
  device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
  let data = buffer.slice(..).get_mapped_range();
  let at = (2 * row + 2 * 4) as usize;
  [data[at], data[at + 1], data[at + 2], data[at + 3]]
}

fn main() {
  let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::VULKAN,
    ..wgpu::InstanceDescriptor::new_without_display_handle()
  });
  let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
  println!("adapter: {:?}", adapter.get_info().name);
  let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
  let new_encoder = || device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
  let green = [0, 255, 0, 255];

  // 1. destroy_image, then upload_image into the slot it freed, in one encoder.
  {
    let (mut renderer, mut resources) = Renderer::new(&device, &RenderTargetConfig { format: wgpu::TextureFormat::Rgba8Unorm, width: SIZE, height: SIZE });
    let mut encoder = new_encoder();
    let red = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(16, [255, 0, 0, 255]));
    queue.submit([encoder.finish()]);
    let mut encoder = new_encoder();
    renderer.destroy_image(&mut resources, &mut encoder, red);
    let id = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(16, green));
    let pixel = draw_and_read(&device, &queue, &mut renderer, &mut resources, encoder, id, 16);
    println!("1. upload after destroy_image in one encoder: drawn {pixel:?}, expected {green:?}");

    // The same with the encoder submitted between the two calls.
    let mut encoder = new_encoder();
    renderer.destroy_image(&mut resources, &mut encoder, id);
    queue.submit([encoder.finish()]);
    let mut encoder = new_encoder();
    let id = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(16, green));
    let pixel = draw_and_read(&device, &queue, &mut renderer, &mut resources, encoder, id, 16);
    println!("   with the encoder submitted after destroy_image: drawn {pixel:?}");
  }

  // 2. An upload that grows the atlas array, then a small upload into the first layer, in one
  //    encoder: the growth's copy of the old layers runs after the small one's write.
  {
    let (mut renderer, mut resources) = Renderer::new(&device, &RenderTargetConfig { format: wgpu::TextureFormat::Rgba8Unorm, width: SIZE, height: SIZE });
    let mut encoder = new_encoder();
    let _first = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(16, [255, 0, 0, 255]));
    queue.submit([encoder.finish()]);
    let atlas = renderer.atlas_texture().size().width;
    println!("   atlas side {atlas}");
    let mut encoder = new_encoder();
    // Larger than the first layer's free space: allocated in a new layer, growing the array.
    let side = u16::try_from(atlas - 8).unwrap_or(u16::MAX);
    let layers = renderer.atlas_texture().size().depth_or_array_layers;
    let _big = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(side, [0, 0, 255, 255]));
    println!("   atlas layers {layers} -> {}", renderer.atlas_texture().size().depth_or_array_layers);
    let id = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(16, green));
    let pixel = draw_and_read(&device, &queue, &mut renderer, &mut resources, encoder, id, 16);
    println!("2. upload after an upload that grew the atlas, in one encoder: drawn {pixel:?}, expected {green:?}");

    // The same with the encoder submitted after the growth.
    let mut encoder = new_encoder();
    let _bigger = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(side, [0, 0, 255, 255]));
    println!("   atlas layers now {}", renderer.atlas_texture().size().depth_or_array_layers);
    queue.submit([encoder.finish()]);
    let mut encoder = new_encoder();
    let id = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(16, green));
    let pixel = draw_and_read(&device, &queue, &mut renderer, &mut resources, encoder, id, 16);
    println!("   with the encoder submitted after the growth: drawn {pixel:?}");
  }
}
