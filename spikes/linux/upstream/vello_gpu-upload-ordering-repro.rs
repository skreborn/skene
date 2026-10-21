//! vello_gpu 0.3.0: whether an image uploaded after `destroy_image` (or after an upload that needs
//! another atlas texture) in the same command encoder draws its pixels. `upload_image` writes
//! pixels with `queue.write_texture`, which runs before every command of the next submitted
//! encoder, while `destroy_image`'s clear is recorded into the caller's encoder.
use vello_common::{
  kurbo::{Affine, Rect},
  paint::{Image, ImageId, ImageSource},
  peniko::{ImageAlphaType, ImageQuality, ImageSampler, color::AlphaColor},
  pixmap::{PixelMetadata, Pixmap},
};
use vello_gpu::{ClearSettings, RenderSize, RenderTargetConfig, Renderer, Resources, Scene, TargetInit, TextureBindings};

const SIZE: u16 = 64;

fn solid(side: u16, pixel: [u8; 4]) -> Pixmap {
  let data = pixel.repeat(usize::from(side) * usize::from(side));
  Pixmap::from_parts(data, side, side, PixelMetadata::new(ImageAlphaType::AlphaPremultiplied, true))
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
  let mut scene = Scene::new(SIZE, SIZE);
  scene.set_transform(Affine::IDENTITY);
  scene.set_paint(Image {
    image: ImageSource::opaque_id(id),
    sampler: ImageSampler { quality: ImageQuality::Low, ..ImageSampler::default() },
  });
  scene.fill_rect(&Rect::new(0., 0., f64::from(side), f64::from(side)));
  let size = u32::from(SIZE);
  let target = device.create_texture(&wgpu::TextureDescriptor {
    label: None,
    size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: wgpu::TextureFormat::Rgba8Unorm,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    view_formats: &[],
  });
  let view = target.create_view(&wgpu::TextureViewDescriptor::default());
  renderer
    .render(
      &scene,
      resources,
      device,
      queue,
      &mut encoder,
      &RenderSize { width: SIZE, height: SIZE },
      &view,
      None,
      &TextureBindings::new(),
      TargetInit::Clear(ClearSettings::Viewport { color: AlphaColor::TRANSPARENT }),
    )
    .expect("render");
  let row = (size * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
  let buffer = device.create_buffer(&wgpu::BufferDescriptor {
    label: None,
    size: u64::from(row * size),
    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
  });
  encoder.copy_texture_to_buffer(
    target.as_image_copy(),
    wgpu::TexelCopyBufferInfo {
      buffer: &buffer,
      layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None },
    },
    wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
  );
  queue.submit([encoder.finish()]);
  buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
  device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
  let data = buffer.slice(..).get_mapped_range().expect("mapped");
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
  let config = RenderTargetConfig { format: wgpu::TextureFormat::Rgba8Unorm, width: SIZE, height: SIZE };

  // 1. destroy_image, then upload_image into the slot it freed, in one encoder.
  {
    let (mut renderer, mut resources) = Renderer::new(&device, &config);
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

  // 2. An upload that needs another atlas texture, then a small upload into the first, in one
  //    encoder.
  {
    let (mut renderer, mut resources) = Renderer::new(&device, &config);
    let mut encoder = new_encoder();
    let first = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(16, [255, 0, 0, 255]));
    queue.submit([encoder.finish()]);
    let _ = first;
    let mut encoder = new_encoder();
    // Larger than the first texture's free space: allocated in a new atlas texture.
    let side = 4096 - 8;
    let _big = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(side, [0, 0, 255, 255]));
    println!("   a second atlas texture exists: {:?}", renderer.atlas_texture(vello_common::multi_atlas::AtlasId::new(1)).size());
    let id = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(16, green));
    let pixel = draw_and_read(&device, &queue, &mut renderer, &mut resources, encoder, id, 16);
    println!("2. upload after an upload that needed another atlas texture, in one encoder: drawn {pixel:?}, expected {green:?}");
    let mut encoder = new_encoder();
    let _bigger = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(side, [0, 0, 255, 255]));
    let id = renderer.upload_image(&mut resources, &device, &queue, &mut encoder, &solid(16, green));
    let pixel = draw_and_read(&device, &queue, &mut renderer, &mut resources, encoder, id, 16);
    println!("   and after a third: drawn {pixel:?}, expected {green:?}");
  }
}
