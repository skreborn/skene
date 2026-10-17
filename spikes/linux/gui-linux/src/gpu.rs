//! The GPU path: a window's frames drawn by vello's hybrid renderer over wgpu (the sparse-strip
//! scene vello_cpu draws, its fine rasterization run on the GPU) and presented through the
//! platform's own swapchain on the window's surface (Vulkan's WSI, on Wayland and on X11).
//!
//! The path is chosen once per window ([`PathChoice`]): where an adapter the choice admits
//! presents to the window's surface, the window paints on the GPU; elsewhere, and from the
//! moment its device is lost, it paints on the CPU, the floor every window has. A software
//! adapter (Mesa's lavapipe) is admitted only where `SKENE_RENDERER=gpu` asks for it, since it
//! runs the same fine rasterization on the CPU more slowly than vello_cpu.

use std::{
  collections::HashMap,
  ffi::c_void,
  num::NonZeroU32,
  ptr::NonNull,
  rc::Rc,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use raw_window_handle::{
  RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, XcbDisplayHandle,
  XcbWindowHandle,
};
use skene_parley::{
  linux::{GlyphMask, MaskPixels},
  render::PlacedGlyph,
};
use vello_common::{
  kurbo::{Affine, BezPath, Rect as KRect, Stroke},
  paint::{Image, ImageId, ImageSource, Tint, TintMode},
  peniko::{Color as PColor, ImageQuality, ImageSampler, color::PremulRgba8},
  pixmap::Pixmap,
};
use vello_hybrid::{RenderSize, RenderTargetConfig, Renderer, Resources, Scene, TextureBindings};

use crate::paint::{Brush, PathChoice, Sink, color};

/// Where a window's surface is, as the platform's own handles.
#[derive(Clone, Copy, Debug)]
pub enum Target {
  /// A Wayland surface: libwayland's display and the surface's proxy.
  Wayland {
    /// The `wl_display`.
    display: NonNull<c_void>,
    /// The `wl_surface`.
    surface: NonNull<c_void>,
  },
  /// An X11 window, through libxcb.
  X11 {
    /// The `xcb_connection_t`.
    connection: NonNull<c_void>,
    /// The screen's number.
    screen: i32,
    /// The window.
    window: NonZeroU32,
  },
}

/// A part of a glyph's mask uploaded to the atlas: its image, and its place and size in the
/// mask, in device pixels. A mask larger than [`Gpu::TILE`] either way is uploaded in parts.
#[derive(Clone, Copy, Debug)]
struct Tile {
  /// The atlas image.
  id: ImageId,
  /// Its leading edge in the mask.
  x: u32,
  /// Its top in the mask.
  y: u32,
  /// Its width.
  width: u32,
  /// Its height.
  height: u32,
}

/// What a GPU frame cost, in microseconds.
#[derive(Clone, Copy, Default, Debug)]
pub struct GpuCost {
  /// Recording the scene.
  pub record: u64,
  /// Rendering it, presenting it and waiting for the GPU to finish.
  pub render: u64,
  /// Whether a glyph found no room in the atlas this frame and went undrawn: the next frame,
  /// which starts the atlas over, draws it.
  pub short: bool,
}

/// The device was lost: the window paints on the CPU from now on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Lost;

/// A window's GPU path: the device, the surface it presents to, vello's hybrid renderer and its
/// scene, the last frame kept for a screenshot, and the glyph images uploaded to the atlas.
pub struct Gpu {
  surface: wgpu::Surface<'static>,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  renderer: Renderer,
  resources: Resources,
  scene: Scene,
  frame: wgpu::Texture,
  glyphs: HashMap<usize, (Rc<GlyphMask>, Vec<Tile>)>,
  area: u64,
  lost: Arc<AtomicBool>,
  _instance: wgpu::Instance,
}

impl Gpu {
  /// The most glyph images the atlas keeps before it starts over.
  const MOST_GLYPHS: usize = 16_384;
  /// The side of vello_hybrid's atlas textures, in device pixels.
  const ATLAS: u64 = 4096;
  /// How many atlas textures vello_hybrid makes at most.
  const ATLASES: u64 = 8;
  /// The atlas pixels glyph images may hold when a frame starts; past them the atlas starts
  /// over.
  const KEEP: u64 = Self::ATLAS * Self::ATLAS * Self::ATLASES / 4;
  /// The atlas pixels glyph images may hold within a frame; a glyph past them is drawn by the
  /// next frame, which starts the atlas over.
  const MOST_AREA: u64 = Self::ATLAS * Self::ATLAS * Self::ATLASES / 2;
  /// The side of the largest part of a mask uploaded as one image, in device pixels.
  const TILE: u32 = 2048;

  /// The GPU path for the surface at `target`, `size` device pixels, where `choice` admits an
  /// adapter that presents to it; why not otherwise.
  ///
  /// # Errors
  ///
  /// Where the choice is the CPU, no adapter presents to the surface, the choice admits none
  /// found, or the device cannot be made.
  ///
  /// # Safety
  ///
  /// The handles in `target` stay valid while the returned path lives.
  pub unsafe fn new(target: Target, size: (u32, u32), choice: PathChoice) -> Result<Self, String> {
    if choice == PathChoice::Cpu {
      return Err("the CPU path was asked for".into());
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
      backends: wgpu::Backends::VULKAN,
      ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let (raw_display_handle, raw_window_handle) = match target {
      Target::Wayland { display, surface } => (
        RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display)),
        RawWindowHandle::Wayland(WaylandWindowHandle::new(surface)),
      ),
      Target::X11 {
        connection,
        screen,
        window,
      } => (
        RawDisplayHandle::Xcb(XcbDisplayHandle::new(Some(connection), screen)),
        RawWindowHandle::Xcb(XcbWindowHandle::new(window)),
      ),
    };
    // SAFETY: the caller keeps the handles valid while the path lives.
    let surface = unsafe {
      instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(raw_display_handle),
        raw_window_handle,
      })
    }
    .map_err(|error| format!("no surface: {error}"))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
      power_preference: wgpu::PowerPreference::HighPerformance,
      force_fallback_adapter: false,
      compatible_surface: Some(&surface),
    }))
    .map_err(|error| format!("no adapter presents to the surface: {error}"))?;
    let info = adapter.get_info();

    if choice == PathChoice::Hardware && info.device_type == wgpu::DeviceType::Cpu {
      return Err(format!(
        "only a software adapter ({}); SKENE_RENDERER=gpu admits it",
        info.name
      ));
    }

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
      label: Some("skene"),
      required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
      ..wgpu::DeviceDescriptor::default()
    }))
    .map_err(|error| format!("no device: {error}"))?;
    let lost = Arc::new(AtomicBool::new(false));

    {
      let lost = Arc::clone(&lost);

      device.set_device_lost_callback(move |reason, message| {
        crate::trace::log(|| format!("[gpu] device lost ({reason:?}): {message}"));
        lost.store(true, Ordering::SeqCst);
      });
    }

    {
      let lost = Arc::clone(&lost);

      device.on_uncaptured_error(Arc::new(move |error| {
        crate::trace::log(|| format!("[gpu] error: {error}"));
        lost.store(true, Ordering::SeqCst);
      }));
    }

    let capabilities = surface.get_capabilities(&adapter);
    let format = [
      wgpu::TextureFormat::Bgra8Unorm,
      wgpu::TextureFormat::Rgba8Unorm,
    ]
    .into_iter()
    .find(|format| capabilities.formats.contains(format))
    .ok_or_else(|| format!("no plain 8-bit format among {:?}", capabilities.formats))?;
    let alpha_mode = [
      wgpu::CompositeAlphaMode::PreMultiplied,
      wgpu::CompositeAlphaMode::Inherit,
      wgpu::CompositeAlphaMode::Opaque,
    ]
    .into_iter()
    .find(|mode| capabilities.alpha_modes.contains(mode))
    .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    let present_mode = [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
      .into_iter()
      .find(|mode| capabilities.present_modes.contains(mode))
      .unwrap_or(wgpu::PresentMode::Fifo);

    if !capabilities.usages.contains(wgpu::TextureUsages::COPY_DST) {
      return Err("the surface takes no copy".into());
    }

    let config = wgpu::SurfaceConfiguration {
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
      format,
      width: size.0.max(1),
      height: size.1.max(1),
      present_mode,
      desired_maximum_frame_latency: 2,
      alpha_mode,
      view_formats: Vec::new(),
    };

    surface.configure(&device, &config);

    let (renderer, resources) = Renderer::new(
      &device,
      &RenderTargetConfig {
        format,
        width: config.width,
        height: config.height,
      },
    );
    let frame = Self::frame_texture(&device, &config);
    let scene = Scene::new(
      u16::try_from(config.width).unwrap_or(u16::MAX),
      u16::try_from(config.height).unwrap_or(u16::MAX),
    );

    crate::trace::log(|| {
      format!(
        "[gpu] {} ({:?}, {:?}) {format:?} {alpha_mode:?} {present_mode:?}",
        info.name, info.device_type, info.backend
      )
    });

    Ok(Self {
      surface,
      device,
      queue,
      config,
      renderer,
      resources,
      scene,
      frame,
      glyphs: HashMap::new(),
      area: 0,
      lost,
      _instance: instance,
    })
  }

  /// Submits what `encoder` holds and starts it afresh: an atlas write queued after this runs
  /// after the clears and copies recorded before it, where one queued before it runs first.
  fn submit_so_far(device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder) {
    let recorded = std::mem::replace(
      encoder,
      device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("skene frame"),
      }),
    );

    queue.submit([recorded.finish()]);
  }

  fn frame_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
      label: Some("skene frame"),
      size: wgpu::Extent3d {
        width: config.width,
        height: config.height,
        depth_or_array_layers: 1,
      },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: config.format,
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
      view_formats: &[],
    })
  }

  /// Whether the device was lost.
  #[must_use]
  pub fn is_lost(&self) -> bool {
    self.lost.load(Ordering::SeqCst)
  }

  /// Loses the device, as a driver reset or a removed GPU would: what a test of the fallback
  /// does.
  pub fn lose(&self) {
    self.device.destroy();
    self.lost.store(true, Ordering::SeqCst);
  }

  /// Draws a whole frame of `size` device pixels through `record`, which draws into the sink in
  /// the scene's order, and presents it.
  ///
  /// # Errors
  ///
  /// Where the device is lost, before or while it draws.
  pub fn paint(
    &mut self,
    size: (u32, u32),
    record: &mut dyn FnMut(&mut dyn Sink),
  ) -> Result<GpuCost, Lost> {
    if self.is_lost() {
      return Err(Lost);
    }

    let started = std::time::Instant::now();
    let (width, height) = (size.0.max(1), size.1.max(1));

    if (width, height) != (self.config.width, self.config.height) {
      self.config.width = width;
      self.config.height = height;
      self.surface.configure(&self.device, &self.config);
      self.frame = Self::frame_texture(&self.device, &self.config);
    }

    let (scene_width, scene_height) = (
      u16::try_from(width).unwrap_or(u16::MAX),
      u16::try_from(height).unwrap_or(u16::MAX),
    );

    if (self.scene.width(), self.scene.height()) == (scene_width, scene_height) {
      self.scene.reset();
    } else {
      self.scene.reset_and_resize(scene_width, scene_height);
    }

    let mut encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("skene frame"),
      });
    let view = self
      .frame
      .create_view(&wgpu::TextureViewDescriptor::default());

    // The frame starts clear: the shadow's margin is translucent.
    drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      label: Some("skene clear"),
      color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        view: &view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
          load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
          store: wgpu::StoreOp::Store,
        },
      })],
      depth_stencil_attachment: None,
      occlusion_query_set: None,
      timestamp_writes: None,
      multiview_mask: None,
    }));

    if self.glyphs.len() >= Self::MOST_GLYPHS || self.area > Self::KEEP {
      for (_, (_, tiles)) in self.glyphs.drain() {
        for tile in tiles {
          self
            .renderer
            .destroy_image(&mut self.resources, &mut encoder, tile.id);
        }
      }

      self.area = 0;
      Self::submit_so_far(&self.device, &self.queue, &mut encoder);
    }

    let mut short = false;

    {
      let mut sink = HybridSink {
        scene: &mut self.scene,
        renderer: &mut self.renderer,
        resources: &mut self.resources,
        device: &self.device,
        queue: &self.queue,
        encoder: &mut encoder,
        glyphs: &mut self.glyphs,
        area: &mut self.area,
        short: &mut short,
      };

      record(&mut sink);
    }

    let recorded = started.elapsed();

    self
      .renderer
      .render(
        &self.scene,
        &mut self.resources,
        &self.device,
        &self.queue,
        &mut encoder,
        &RenderSize { width, height },
        &view,
        &TextureBindings::new(),
      )
      .map_err(|_| Lost)?;

    let texture = match self.surface.get_current_texture() {
      wgpu::CurrentSurfaceTexture::Success(texture)
      | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
      wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
        self.surface.configure(&self.device, &self.config);

        match self.surface.get_current_texture() {
          wgpu::CurrentSurfaceTexture::Success(texture)
          | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
          _ => return Err(Lost),
        }
      }
      _ => {
        self.queue.submit([encoder.finish()]);

        return Err(Lost);
      }
    };

    encoder.copy_texture_to_texture(
      self.frame.as_image_copy(),
      texture.texture.as_image_copy(),
      wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
      },
    );
    self.queue.submit([encoder.finish()]);
    texture.present();

    if self
      .device
      .poll(wgpu::PollType::wait_indefinitely())
      .is_err()
      || self.is_lost()
    {
      return Err(Lost);
    }

    #[expect(clippy::cast_possible_truncation, reason = "microseconds")]
    Ok(GpuCost {
      record: recorded.as_micros() as u64,
      render: (started.elapsed() - recorded).as_micros() as u64,
      short,
    })
  }

  /// The last frame drawn, read back: premultiplied RGBA, row by row, `width` by `height`.
  #[must_use]
  pub fn read_back(&self) -> Option<(u32, u32, Vec<[u8; 4]>)> {
    if self.is_lost() {
      return None;
    }

    let (width, height) = (self.config.width, self.config.height);
    let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("skene read back"),
      size: u64::from(row) * u64::from(height),
      usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
      mapped_at_creation: false,
    });
    let mut encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    encoder.copy_texture_to_buffer(
      self.frame.as_image_copy(),
      wgpu::TexelCopyBufferInfo {
        buffer: &buffer,
        layout: wgpu::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(row),
          rows_per_image: None,
        },
      },
      wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
      },
    );
    self.queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;

    let bgra = self.config.format == wgpu::TextureFormat::Bgra8Unorm;
    let data = buffer.slice(..).get_mapped_range();
    let mut pixels = Vec::with_capacity(width as usize * height as usize);

    for line in data.chunks(row as usize).take(height as usize) {
      for pixel in line[..width as usize * 4].as_chunks::<4>().0 {
        let [first, green, third, alpha] = *pixel;

        pixels.push(if bgra {
          [third, green, first, alpha]
        } else {
          [first, green, third, alpha]
        });
      }
    }

    drop(data);
    buffer.unmap();

    Some((width, height, pixels))
  }
}

/// vello's hybrid scene as a [`Sink`], uploading each glyph's image to the renderer's atlas the
/// first time it is drawn.
struct HybridSink<'a> {
  scene: &'a mut Scene,
  renderer: &'a mut Renderer,
  resources: &'a mut Resources,
  device: &'a wgpu::Device,
  queue: &'a wgpu::Queue,
  encoder: &'a mut wgpu::CommandEncoder,
  glyphs: &'a mut HashMap<usize, (Rc<GlyphMask>, Vec<Tile>)>,
  area: &'a mut u64,
  short: &'a mut bool,
}

impl HybridSink<'_> {
  fn brush(&mut self, transform: Affine, brush: &Brush) {
    self.scene.set_transform(transform);

    match brush {
      Brush::Solid(color) => {
        self.scene.set_paint(*color);
        self.scene.reset_paint_transform();
      }
      Brush::Gradient(gradient, placed) => {
        self.scene.set_paint(gradient.clone());
        self
          .scene
          .set_paint_transform(transform.inverse() * *placed);
      }
    }
  }

  /// The atlas images of `mask`, uploaded where it is new, in parts no larger than
  /// [`Gpu::TILE`] either way; `None`, and the frame short, where they would take the atlas past
  /// [`Gpu::MOST_AREA`]. A part that grows the atlas submits what the frame recorded so far
  /// ([`Gpu::submit_so_far`]).
  fn image(&mut self, mask: &Rc<GlyphMask>) -> Option<Vec<Tile>> {
    let key = Rc::as_ptr(mask) as usize;

    if let Some((_, tiles)) = self.glyphs.get(&key) {
      return Some(tiles.clone());
    }

    let area = u64::from(mask.width) * u64::from(mask.height);

    if *self.area + area > Gpu::MOST_AREA {
      *self.short = true;

      return None;
    }

    let mut tiles = Vec::new();

    for y in (0..mask.height).step_by(Gpu::TILE as usize) {
      for x in (0..mask.width).step_by(Gpu::TILE as usize) {
        let (width, height) = (
          Gpu::TILE.min(mask.width - x),
          Gpu::TILE.min(mask.height - y),
        );
        let mut pixels = Vec::with_capacity(width as usize * height as usize);

        for row in y..y + height {
          let start = (row * mask.width + x) as usize;
          let end = start + width as usize;

          match &mask.pixels {
            MaskPixels::Alpha(alpha) => {
              pixels.extend(alpha[start..end].iter().map(|&a| PremulRgba8 {
                r: 0,
                g: 0,
                b: 0,
                a,
              }));
            }
            MaskPixels::Color(rgba) => pixels.extend(
              rgba[start * 4..end * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|&[r, g, b, a]| PremulRgba8 { r, g, b, a }),
            ),
          }
        }

        let pixmap = Pixmap::from_parts(
          pixels,
          u16::try_from(width).unwrap_or(1),
          u16::try_from(height).unwrap_or(1),
        );
        let layers = self.renderer.atlas_texture().size().depth_or_array_layers;
        let id = self.renderer.upload_image(
          self.resources,
          self.device,
          self.queue,
          self.encoder,
          &pixmap,
        );

        if self.renderer.atlas_texture().size().depth_or_array_layers != layers {
          Gpu::submit_so_far(self.device, self.queue, self.encoder);
        }

        tiles.push(Tile {
          id,
          x,
          y,
          width,
          height,
        });
      }
    }

    *self.area += area;
    self.glyphs.insert(key, (Rc::clone(mask), tiles.clone()));

    Some(tiles)
  }
}

impl Sink for HybridSink<'_> {
  fn fill(&mut self, path: &BezPath, transform: Affine, brush: &Brush) {
    self.brush(transform, brush);
    self.scene.fill_path(path);
  }

  fn fill_rect(&mut self, rect: KRect, transform: Affine, brush: &Brush) {
    self.brush(transform, brush);
    self.scene.fill_rect(&rect);
  }

  fn stroke(&mut self, path: &BezPath, stroke: &Stroke, transform: Affine, brush: &Brush) {
    self.brush(transform, brush);
    self.scene.set_stroke(stroke.clone());
    self.scene.stroke_path(path);
  }

  fn blurred_rect(
    &mut self,
    rect: KRect,
    radius: f32,
    deviation: f32,
    invert: bool,
    transform: Affine,
    color: PColor,
  ) {
    self.brush(transform, &Brush::Solid(color));
    self
      .scene
      .fill_blurred_rounded_rect(&rect, radius, deviation, invert);
  }

  fn push_clip(&mut self, path: &BezPath, transform: Affine) {
    self.scene.set_transform(transform);
    self.scene.push_clip_layer(path);
  }

  fn push_opacity(&mut self, opacity: f32) {
    self.scene.push_opacity_layer(opacity);
  }

  fn pop(&mut self) {
    self.scene.pop_layer();
  }

  fn glyph(&mut self, glyph: &PlacedGlyph) {
    if glyph.mask.width == 0 || glyph.mask.height == 0 {
      return;
    }

    let Some(tiles) = self.image(&glyph.mask) else {
      return;
    };
    let tint = matches!(glyph.mask.pixels, MaskPixels::Alpha(_)).then(|| Tint {
      color: color(glyph.color),
      mode: TintMode::AlphaMask,
    });

    for tile in tiles {
      #[expect(clippy::cast_precision_loss, reason = "device pixels")]
      self.scene.set_transform(Affine::translate((
        glyph.left as f64 + f64::from(tile.x),
        glyph.top as f64 + f64::from(tile.y),
      )));
      self.scene.reset_paint_transform();
      self.scene.set_paint(Image {
        image: ImageSource::opaque_id(tile.id),
        sampler: ImageSampler {
          quality: ImageQuality::Low,
          ..ImageSampler::default()
        },
      });
      self.scene.set_tint(tint);
      self.scene.fill_rect(&KRect::new(
        0.,
        0.,
        f64::from(tile.width),
        f64::from(tile.height),
      ));
      self.scene.reset_tint();
    }
  }
}
