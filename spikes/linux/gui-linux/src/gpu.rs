//! The GPU path: a window's frames drawn by vello's GPU renderer over wgpu (the sparse-strip
//! scene vello_cpu draws, its fine rasterization run on the GPU) and presented through the
//! platform's own swapchain on the window's surface (Vulkan's WSI, on Wayland and on X11), and
//! its popups' on theirs, one device shared by the window's surfaces.
//!
//! The path is chosen once per window ([`PathChoice`]): where an adapter the choice admits
//! presents to the window's surface, the window paints on the GPU, and so does each popup it
//! opens where the adapter presents to the popup's surface; elsewhere, and from the moment the
//! device is lost, it paints on the CPU, the floor every window has. A lost device wakes the
//! window's loop, which paints the window and its popup again on the CPU at once. A software
//! adapter (Mesa's lavapipe) is admitted only where `SKENE_RENDERER=gpu` asks for it, since it
//! runs the same fine rasterization on the CPU more slowly than vello_cpu.
//!
//! A frame draws the part of the window that changed alone, as the CPU path does: a band from
//! the frame's leading edge, its top on vello's tile grid, drawn as the whole frame is but cut to
//! the band, into a texture of the frame's size, and copied into the window's whole frame, which
//! the surface is given whole.

use std::{
  cell::RefCell,
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
  kurbo::{Affine, BezPath, Rect as KRect, Shape as _, Stroke},
  paint::{Image, ImageId, ImageSource, Tint, TintMode},
  peniko::{self, Color as PColor, ImageAlphaType, ImageQuality, ImageSampler},
  pixmap::{PixelMetadata, Pixmap},
};
use vello_gpu::{
  ClearSettings, RenderSize, RenderTargetConfig, Renderer, Resources, Scene, TargetInit,
  TextureBindings,
};

use crate::paint::{Brush, BrushImages, CastShadow, ImageKey, PathChoice, Sink, color};

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

/// A part of a frame, in device pixels: its leading edge, its top, its width and its height.
pub type Band = (u32, u32, u32, u32);

/// What a window's surfaces paint with on the GPU, the window's and its popups' alike: vello's
/// GPU renderer, its resources and the glyph and brush images in its atlas, the surfaces'
/// format, the adapter's device and queue, and whether the device was lost.
struct Shared {
  renderer: Renderer,
  resources: Resources,
  glyphs: HashMap<usize, (Rc<GlyphMask>, Vec<Tile>)>,
  images: HashMap<ImageKey, ImageId>,
  area: u64,
  /// Whether the last frame was short of a glyph, so the next starts the atlas over.
  starved: bool,
  format: wgpu::TextureFormat,
  lost: Arc<AtomicBool>,
  queue: wgpu::Queue,
  device: wgpu::Device,
  adapter: wgpu::Adapter,
  instance: wgpu::Instance,
}

/// A surface's GPU path: the surface it presents to, its configuration and vello's GPU scene,
/// the whole frame kept between frames (what a frame drawing a part of it copies into, and a
/// screenshot reads), the texture a part is drawn into, the depth buffer every frame is drawn
/// with, and the device it shares with the window's other surfaces.
pub struct Gpu {
  surface: wgpu::Surface<'static>,
  config: wgpu::SurfaceConfiguration,
  scene: Scene,
  frame: wgpu::Texture,
  part: Option<wgpu::Texture>,
  depth: wgpu::TextureView,
  whole: bool,
  shared: Rc<RefCell<Shared>>,
}

impl Gpu {
  /// The most glyph images the atlas keeps before it starts over.
  const MOST_GLYPHS: usize = 16_384;
  /// The side of vello_gpu's atlas textures, in device pixels.
  const ATLAS: u64 = 4096;
  /// How many atlas textures vello_gpu makes at most.
  const ATLASES: u64 = 8;
  /// The atlas pixels glyph images may hold when a frame starts; past them the atlas starts
  /// over.
  const KEEP: u64 = Self::ATLAS * Self::ATLAS * Self::ATLASES / 4;
  /// The atlas pixels glyph images may hold within a frame; a glyph past them is drawn by the
  /// next frame, which starts the atlas over.
  const MOST_AREA: u64 = Self::ATLAS * Self::ATLAS * Self::ATLASES / 2;
  /// The atlas pixels glyph images may hold within a frame that started with the atlas empty:
  /// all but what the parts' packing may leave unused. A glyph past them is not drawn on the
  /// GPU, and no frame after it asks again.
  const MOST_AREA_FRESH: u64 = Self::ATLAS * Self::ATLAS * Self::ATLASES * 3 / 4;
  /// The side of the largest part of a mask uploaded as one image, in device pixels.
  const TILE: u32 = 2048;

  /// The surface at `target`, on `instance`.
  ///
  /// # Safety
  ///
  /// The handles in `target` stay valid while the surface lives.
  unsafe fn surface(
    instance: &wgpu::Instance,
    target: Target,
  ) -> Result<wgpu::Surface<'static>, String> {
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

    // SAFETY: the caller keeps the handles valid while the surface lives.
    unsafe {
      instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(raw_display_handle),
        raw_window_handle,
      })
    }
    .map_err(|error| format!("no surface: {error}"))
  }

  /// The GPU path for the surface at `target`, `size` device pixels, where `choice` admits an
  /// adapter that presents to it; why not otherwise. A lost device posts
  /// [`crate::runtime::Message::DeviceLost`] to this thread's loop.
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
    // SAFETY: the caller keeps the handles valid while the path lives.
    let surface = unsafe { Self::surface(&instance, target) }?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
      power_preference: wgpu::PowerPreference::HighPerformance,
      force_fallback_adapter: false,
      compatible_surface: Some(&surface),
      apply_limit_buckets: false,
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
      let inbox = crate::runtime::inbox();

      device.set_device_lost_callback(move |reason, message| {
        crate::trace::log(|| format!("[gpu] device lost ({reason:?}): {message}"));
        lost.store(true, Ordering::SeqCst);
        inbox.send(crate::runtime::Message::DeviceLost);
      });
    }

    {
      let lost = Arc::clone(&lost);
      let inbox = crate::runtime::inbox();

      device.on_uncaptured_error(Arc::new(move |error| {
        crate::trace::log(|| format!("[gpu] error: {error}"));
        lost.store(true, Ordering::SeqCst);
        inbox.send(crate::runtime::Message::DeviceLost);
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
    let (renderer, resources) = Renderer::new(
      &device,
      &RenderTargetConfig {
        format,
        width: Self::side(size.0),
        height: Self::side(size.1),
      },
    );

    crate::trace::log(|| {
      format!(
        "[gpu] {} ({:?}, {:?})",
        info.name, info.device_type, info.backend
      )
    });

    let shared = Rc::new(RefCell::new(Shared {
      renderer,
      resources,
      glyphs: HashMap::new(),
      images: HashMap::new(),
      area: 0,
      starved: false,
      format,
      lost,
      queue,
      device,
      adapter,
      instance,
    }));

    Self::on(shared, surface, size)
  }

  /// The GPU path for the surface at `target`, `size` device pixels, on this path's device:
  /// a popup's, the window's device shared; why not where the adapter does not present to it.
  ///
  /// # Errors
  ///
  /// Where the adapter does not present to the surface, or not in this path's format.
  ///
  /// # Safety
  ///
  /// The handles in `target` stay valid while the returned path lives.
  pub unsafe fn share(&self, target: Target, size: (u32, u32)) -> Result<Self, String> {
    let surface = {
      let shared = self.shared.borrow();
      // SAFETY: the caller keeps the handles valid while the path lives.
      let surface = unsafe { Self::surface(&shared.instance, target) }?;

      if !shared.adapter.is_surface_supported(&surface) {
        return Err("the window's adapter does not present to the surface".into());
      }

      surface
    };

    Self::on(Rc::clone(&self.shared), surface, size)
  }

  /// The path presenting to `surface`, `size` device pixels, on `shared`.
  fn on(
    shared: Rc<RefCell<Shared>>,
    surface: wgpu::Surface<'static>,
    size: (u32, u32),
  ) -> Result<Self, String> {
    let config = {
      let device = shared.borrow();
      let capabilities = surface.get_capabilities(&device.adapter);

      if !capabilities.formats.contains(&device.format) {
        return Err(format!(
          "the surface takes no {:?}, only {:?}",
          device.format, capabilities.formats
        ));
      }

      if !capabilities.usages.contains(wgpu::TextureUsages::COPY_DST) {
        return Err("the surface takes no copy".into());
      }

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
      let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
        format: device.format,
        width: size.0.max(1),
        height: size.1.max(1),
        present_mode,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: Vec::new(),
        color_space: wgpu::SurfaceColorSpace::Auto,
      };

      surface.configure(&device.device, &config);
      crate::trace::log(|| {
        format!(
          "[gpu] surface {}x{} {:?} {alpha_mode:?} {present_mode:?}",
          config.width, config.height, config.format
        )
      });

      config
    };
    let frame = Self::texture(
      &shared.borrow().device,
      &config,
      (config.width, config.height),
    );
    let depth = Self::depth(&shared.borrow().device, (config.width, config.height));
    let scene = Scene::new(Self::side(config.width), Self::side(config.height));

    Ok(Self {
      surface,
      config,
      scene,
      frame,
      part: None,
      depth,
      whole: true,
      shared,
    })
  }

  /// A side of a frame, in device pixels, as vello_gpu measures its scenes and targets.
  fn side(pixels: u32) -> u16 {
    u16::try_from(pixels.max(1)).unwrap_or(u16::MAX)
  }

  /// The depth buffer a frame of `size` device pixels is drawn with.
  fn depth(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    Renderer::create_depth_texture_view(
      device,
      &RenderSize {
        width: Self::side(size.0),
        height: Self::side(size.1),
      },
    )
  }

  /// Submits what `encoder` holds and starts it afresh: an atlas write queued after this runs
  /// after the clears recorded before it, where one queued before it runs first.
  fn submit_so_far(device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder) {
    let recorded = std::mem::replace(
      encoder,
      device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("skene frame"),
      }),
    );

    queue.submit([recorded.finish()]);
  }

  /// A texture of `size` device pixels in the surface's format that frames and their parts are
  /// drawn into, copied into and copied from.
  fn texture(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    size: (u32, u32),
  ) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
      label: Some("skene frame"),
      size: wgpu::Extent3d {
        width: size.0,
        height: size.1,
        depth_or_array_layers: 1,
      },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: config.format,
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::COPY_SRC
        | wgpu::TextureUsages::COPY_DST,
      view_formats: &[],
    })
  }

  /// Whether the device was lost.
  #[must_use]
  pub fn is_lost(&self) -> bool {
    self.shared.borrow().lost.load(Ordering::SeqCst)
  }

  /// Loses the device, as a driver reset or a removed GPU would, unknown to the window: what a
  /// test of the fallback does. The device's loss is reported as a real one is, through its
  /// callback, which wakes the window's loop.
  pub fn lose(&self) {
    let shared = self.shared.borrow();

    shared.device.destroy();
    let _ = shared.device.poll(wgpu::PollType::wait_indefinitely());
  }

  /// Whether the next frame of `size` device pixels is to be drawn whole: the surface's first,
  /// one of another size, or one after the device's atlas started over.
  #[must_use]
  pub fn needs_whole(&self, size: (u32, u32)) -> bool {
    self.whole || (size.0.max(1), size.1.max(1)) != (self.config.width, self.config.height)
  }

  /// Draws the part `band` (device pixels, [`Gpu::needs_whole`] saying where it is to be the
  /// whole) of a frame of `size` device pixels through `record`, which draws into the sink in
  /// the scene's order in the frame's own device pixels, and presents the frame whole.
  ///
  /// A part is drawn as the whole frame is, in a scene and a texture of the frame's size, cut to
  /// the band, and copied into the frame: what the GPU computes of a pixel (an edge's
  /// coverage, a paint's sample) does not depend on what else is drawn.
  ///
  /// # Errors
  ///
  /// Where the device is lost, before or while it draws.
  pub fn paint(
    &mut self,
    size: (u32, u32),
    band: Band,
    record: &mut dyn FnMut(&mut dyn Sink),
  ) -> Result<GpuCost, Lost> {
    if self.is_lost() {
      return Err(Lost);
    }

    let started = std::time::Instant::now();
    let (width, height) = (size.0.max(1), size.1.max(1));
    let shared = Rc::clone(&self.shared);
    let mut shared = shared.borrow_mut();
    let shared = &mut *shared;

    if (width, height) != (self.config.width, self.config.height) {
      self.config.width = width;
      self.config.height = height;
      self.surface.configure(&shared.device, &self.config);
      self.frame = Self::texture(&shared.device, &self.config, (width, height));
      self.part = None;
      self.depth = Self::depth(&shared.device, (width, height));
    }

    let (left, top, band_width, band_height) = band;
    let (left, top) = (left.min(width - 1), top.min(height - 1));
    let (band_width, band_height) = (
      band_width.clamp(1, width - left),
      band_height.clamp(1, height - top),
    );
    let whole = (left, top, band_width, band_height) == (0, 0, width, height);
    let (scene_width, scene_height) = (Self::side(width), Self::side(height));

    if (self.scene.width(), self.scene.height()) == (scene_width, scene_height) {
      self.scene.reset();
    } else {
      self.scene.reset_and_resize(scene_width, scene_height);
    }

    let mut encoder = shared
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("skene frame"),
      });
    let mut short = false;

    if shared.starved || shared.glyphs.len() >= Self::MOST_GLYPHS || shared.area > Self::KEEP {
      let Shared {
        glyphs,
        images,
        renderer,
        resources,
        ..
      } = &mut *shared;
      let ids = glyphs
        .drain()
        .flat_map(|(_, (_, tiles))| tiles.into_iter().map(|tile| tile.id))
        .chain(images.drain().map(|(_, id)| id));

      for id in ids {
        renderer.destroy_image(resources, &mut encoder, id);
      }

      shared.area = 0;
      Self::submit_so_far(&shared.device, &shared.queue, &mut encoder);
    }

    let fresh = shared.area == 0;

    // Every frame is cut to its band, the whole frame's too, so a rectangle is drawn alike
    // either way.
    self.scene.reset_transform();
    self.scene.push_clip_path(
      &KRect::new(
        f64::from(left),
        f64::from(top),
        f64::from(left + band_width),
        f64::from(top + band_height),
      )
      .to_path(0.1),
    );

    {
      let mut sink = GpuSink {
        scene: &mut self.scene,
        renderer: &mut shared.renderer,
        resources: &mut shared.resources,
        device: &shared.device,
        queue: &shared.queue,
        encoder: &mut encoder,
        glyphs: &mut shared.glyphs,
        images: &mut shared.images,
        area: &mut shared.area,
        most: if fresh {
          Self::MOST_AREA_FRESH
        } else {
          Self::MOST_AREA
        },
        short: &mut short,
      };

      record(&mut sink);
    }

    self.scene.pop_clip();

    let recorded = started.elapsed();
    let target = if whole {
      self.frame.clone()
    } else {
      self
        .part
        .get_or_insert_with(|| Self::texture(&shared.device, &self.config, (width, height)))
        .clone()
    };
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    // Drawn on a clear texture: the shadow's margin is translucent.
    shared
      .renderer
      .render(
        &self.scene,
        &mut shared.resources,
        &shared.device,
        &shared.queue,
        &mut encoder,
        &RenderSize {
          width: scene_width,
          height: scene_height,
        },
        &view,
        Some(&self.depth),
        &TextureBindings::new(),
        TargetInit::Clear(ClearSettings::Viewport {
          color: PColor::TRANSPARENT,
        }),
      )
      .map_err(|_| Lost)?;

    if !whole {
      let origin = wgpu::Origin3d {
        x: left,
        y: top,
        z: 0,
      };

      encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
          texture: &target,
          mip_level: 0,
          origin,
          aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
          texture: &self.frame,
          mip_level: 0,
          origin,
          aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
          width: band_width,
          height: band_height,
          depth_or_array_layers: 1,
        },
      );
    }

    let texture = match self.surface.get_current_texture() {
      wgpu::CurrentSurfaceTexture::Success(texture)
      | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
      wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
        self.surface.configure(&shared.device, &self.config);

        match self.surface.get_current_texture() {
          wgpu::CurrentSurfaceTexture::Success(texture)
          | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
          _ => return Err(Lost),
        }
      }
      _ => {
        shared.queue.submit([encoder.finish()]);

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
    shared.queue.submit([encoder.finish()]);
    shared.queue.present(texture);

    if shared
      .device
      .poll(wgpu::PollType::wait_indefinitely())
      .is_err()
      || shared.lost.load(Ordering::SeqCst)
    {
      return Err(Lost);
    }

    // A frame short of a glyph is drawn again whole once the atlas starts over, unless it
    // started over already: then the glyph is larger than the atlas holds.
    let retry = short && !fresh;

    self.whole = retry;
    shared.starved = retry;

    if short && fresh {
      crate::trace::log(|| "[gpu] a glyph larger than the atlas holds is not drawn".to_owned());
    }

    #[expect(clippy::cast_possible_truncation, reason = "microseconds")]
    Ok(GpuCost {
      record: recorded.as_micros() as u64,
      render: (started.elapsed() - recorded).as_micros() as u64,
      short: retry,
    })
  }

  /// The last frame drawn, read back: premultiplied RGBA, row by row, `width` by `height`.
  #[must_use]
  pub fn read_back(&self) -> Option<(u32, u32, Vec<[u8; 4]>)> {
    if self.is_lost() {
      return None;
    }

    let shared = self.shared.borrow();
    let (width, height) = (self.config.width, self.config.height);
    let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = shared.device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("skene read back"),
      size: u64::from(row) * u64::from(height),
      usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
      mapped_at_creation: false,
    });
    let mut encoder = shared
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
    shared.queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    shared
      .device
      .poll(wgpu::PollType::wait_indefinitely())
      .ok()?;

    let bgra = self.config.format == wgpu::TextureFormat::Bgra8Unorm;
    let data = buffer.slice(..).get_mapped_range().ok()?;
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

/// vello's GPU scene as a [`Sink`], uploading each glyph's image to the renderer's atlas the
/// first time it is drawn.
struct GpuSink<'a> {
  scene: &'a mut Scene,
  renderer: &'a mut Renderer,
  resources: &'a mut Resources,
  device: &'a wgpu::Device,
  queue: &'a wgpu::Queue,
  encoder: &'a mut wgpu::CommandEncoder,
  glyphs: &'a mut HashMap<usize, (Rc<GlyphMask>, Vec<Tile>)>,
  images: &'a mut HashMap<ImageKey, ImageId>,
  area: &'a mut u64,
  most: u64,
  short: &'a mut bool,
}

impl GpuSink<'_> {
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
      Brush::Image(brush, placed) => match self.brush_image(brush) {
        Some(id) => {
          self.scene.set_paint(Image {
            image: ImageSource::opaque_id(id),
            sampler: brush.sampler,
          });
          self
            .scene
            .set_paint_transform(transform.inverse() * *placed);
        }
        None => {
          self.scene.set_paint(PColor::TRANSPARENT);
          self.scene.reset_paint_transform();
        }
      },
    }
  }

  /// The atlas image of the image brush `brush`, uploaded where it is new; `None` where vello
  /// reads no pixels of its format, it is larger than [`Gpu::TILE`] either way, or it would
  /// take the atlas past what a frame may hold (the frame then short).
  fn brush_image(&mut self, brush: &peniko::ImageBrush) -> Option<ImageId> {
    let key = BrushImages::key(brush);

    if let Some(id) = self.images.get(&key) {
      return Some(*id);
    }

    let (width, height) = (brush.image.width, brush.image.height);
    let readable = matches!(
      brush.image.format,
      peniko::ImageFormat::Rgba8 | peniko::ImageFormat::Bgra8
    ) && width <= Gpu::TILE
      && height <= Gpu::TILE
      && brush.image.data.data().len() >= width as usize * height as usize * 4;

    if !readable {
      return None;
    }

    let area = u64::from(width) * u64::from(height);

    if *self.area + area > self.most {
      *self.short = true;

      return None;
    }

    let ImageSource::Pixmap(pixmap) = ImageSource::from_peniko_image_data(&brush.image) else {
      return None;
    };
    let id = self.renderer.upload_image(
      self.resources,
      self.device,
      self.queue,
      self.encoder,
      &pixmap,
    );

    *self.area += area;
    self.images.insert(key, id);

    Some(id)
  }

  /// The atlas images of `mask`, uploaded where it is new, in parts no larger than
  /// [`Gpu::TILE`] either way; `None`, and the frame short, where they would take the atlas past
  /// what a frame may hold.
  fn image(&mut self, mask: &Rc<GlyphMask>) -> Option<Vec<Tile>> {
    let key = Rc::as_ptr(mask) as usize;

    if let Some((_, tiles)) = self.glyphs.get(&key) {
      return Some(tiles.clone());
    }

    let area = u64::from(mask.width) * u64::from(mask.height);

    if *self.area + area > self.most {
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
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);

        for row in y..y + height {
          let start = (row * mask.width + x) as usize;
          let end = start + width as usize;

          match &mask.pixels {
            MaskPixels::Alpha(alpha) => {
              pixels.extend(alpha[start..end].iter().flat_map(|&a| [0, 0, 0, a]));
            }
            MaskPixels::Color(rgba) => pixels.extend_from_slice(&rgba[start * 4..end * 4]),
          }
        }

        let pixmap = Pixmap::from_parts(
          pixels,
          u16::try_from(width).unwrap_or(1),
          u16::try_from(height).unwrap_or(1),
          PixelMetadata::new(ImageAlphaType::AlphaPremultiplied, true),
        );
        let id = self.renderer.upload_image(
          self.resources,
          self.device,
          self.queue,
          self.encoder,
          &pixmap,
        );

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

impl Sink for GpuSink<'_> {
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

  fn shadow(&mut self, shadow: &CastShadow, transform: Affine) {
    self.scene.set_transform(transform);
    self
      .scene
      .set_fill_rule(vello_common::peniko::Fill::EvenOdd);
    self.scene.push_clip_path(&shadow.outside());
    self
      .scene
      .set_fill_rule(vello_common::peniko::Fill::NonZero);
    self.brush(transform, &Brush::Solid(shadow.color));
    self
      .scene
      .fill_blurred_rounded_rect(&shadow.cast, shadow.radius, shadow.deviation, false);
    self.scene.pop_clip();
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
