// Copyright 2022 the Vello Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Vello is a compute-centric 2D graphics rendering engine written in Rust,
//! using [`wgpu`].
//! It efficiently draws large 2D scenes with interactive or near-interactive
//! performance.
//!
//! ![image](https://github.com/linebender/vello/assets/8573618/cc2b742e-2135-4b70-8051-c49aeddb5d19)
//!
//!
//! ## Motivation
//!
//! Vello is meant to fill the same place in the graphics stack as other vector graphics renderers like [Skia](https://skia.org/), [Cairo](https://www.cairographics.org/), and its predecessor project [Piet](https://www.cairographics.org/).
//! On a basic level, that means it provides tools to render shapes, images, gradients, texts, etc, using a PostScript-inspired API, the same that powers SVG files and [the browser `<canvas>` element](https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D).
//!
//! Vello's selling point is that it gets better performance than other renderers by better leveraging the GPU.
//! In traditional PostScript-style renderers, some steps of the render process like sorting and clipping either need to be handled in the CPU or done through the use of intermediary textures.
//! Vello avoids this by using prefix-scan algorithms to parallelize work that usually needs to happen in sequence, so that work can be offloaded to the GPU with minimal use of temporary buffers.
//!
//! This means that Vello needs a GPU with support for compute shaders to run.
//!
//!
//! ## Getting started
//!
//! Vello is meant to be integrated deep in UI render stacks.
//! While drawing in a Vello [`Scene`] is easy, actually rendering that scene to a surface setting up a wgpu context, which is a non-trivial task.
//!
//! To use Vello as the renderer for your PDF reader / GUI toolkit / etc, your code will have to look roughly like this:
//!
//! ```ignore
//! // Initialize wgpu and get handles
//! let (width, height) = ...;
//! let device: wgpu::Device = ...;
//! let queue: wgpu::Queue = ...;
//! let mut renderer = Renderer::new(
//!    &device,
//!    RendererOptions {
//!       use_cpu: false,
//!       antialiasing_support: vello::AaSupport::all(),
//!       num_init_threads: NonZeroUsize::new(1),
//!    },
//! ).expect("Failed to create renderer");
//!
//! // Create scene and draw stuff in it
//! let mut scene = vello::Scene::new();
//! scene.fill(
//!    vello::peniko::Fill::NonZero,
//!    vello::Affine::IDENTITY,
//!    vello::Color::from_rgb8(242, 140, 168),
//!    None,
//!    &vello::Circle::new((420.0, 200.0), 120.0),
//! );
//!
//! // Draw more stuff
//! scene.push_layer(...);
//! scene.fill(...);
//! scene.stroke(...);
//! scene.pop_layer(...);
//!
//! let texture = device.create_texture(&...);
//! // Render to a wgpu Texture
//! renderer
//!    .render_to_texture(
//!       &device,
//!       &queue,
//!       &scene,
//!       &texture,
//!       &vello::RenderParams {
//!          base_color: palette::css::BLACK, // Background color
//!          width,
//!          height,
//!          antialiasing_method: AaConfig::Msaa16,
//!       },
//!    )
//!    .expect("Failed to render to a texture");
//! // Do things with surface texture, such as blitting it to the Surface using
//! // wgpu::util::TextureBlitter.
//! ```
//!
//! See the [`research/examples/`](https://github.com/linebender/vello/tree/main/research/examples) folder to see how that code integrates with frameworks like winit.

// LINEBENDER LINT SET - lib.rs - v2
// See https://linebender.org/wiki/canonical-lints/
// These lints aren't included in Cargo.toml because they
// shouldn't apply to examples and tests
#![warn(unused_crate_dependencies)]
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]
// The following lints are part of the Linebender standard set,
// but resolving them has been deferred for now.
// Feel free to send a PR that solves one or more of these.
// Need to allow instead of expect until Rust 1.83 https://github.com/rust-lang/rust/pull/130025
#![allow(missing_docs, reason = "We have many as-yet undocumented items.")]
#![expect(
    missing_debug_implementations,
    clippy::cast_possible_truncation,
    clippy::missing_assert_message,
    reason = "Deferred"
)]
#![allow(
    clippy::todo,
    unreachable_pub,
    unnameable_types,
    reason = "Deferred, only apply in some feature sets so not expect"
)]

mod debug;
mod recording;
mod render;
mod scene;
mod shaders;

#[cfg(feature = "wgpu")]
pub mod util;
#[cfg(feature = "wgpu")]
mod wgpu_engine;

pub mod low_level {
    //! Utilities which can be used to create an alternative Vello renderer to [`Renderer`][crate::Renderer].
    //!
    //! These APIs have not been carefully designed, and might not be powerful enough for this use case.

    pub use crate::debug::DebugLayers;
    pub use crate::recording::{
        BindType, BufferProxy, Command, ImageFormat, ImageProxy, Recording, ResourceId,
        ResourceProxy, ShaderId,
    };
    pub use crate::render::Render;
    pub use crate::shaders::FullShaders;

    pub use vello_encoding;
}
/// Styling and composition primitives.
pub use peniko;
/// 2D geometry, with a focus on curves.
pub use peniko::kurbo;

#[cfg(feature = "wgpu")]
use peniko::ImageData;
#[cfg(feature = "wgpu")]
pub use wgpu;

pub use scene::{DrawGlyphs, Scene};
pub use vello_encoding::{FontEmbolden, Glyph, NormalizedCoord};

use low_level::ShaderId;
#[cfg(feature = "wgpu")]
use low_level::{BufferProxy, FullShaders, Recording, Render};
use thiserror::Error;

#[cfg(feature = "wgpu")]
use debug::DebugLayers;
#[cfg(feature = "wgpu")]
use vello_encoding::{BumpAllocators, LineSoup, PathSegment, Resolver, SegmentCount, Tile};
#[cfg(feature = "wgpu")]
use wgpu_engine::{ExternalResource, WgpuEngine};

#[cfg(feature = "wgpu")]
use std::{
    num::NonZeroUsize,
    sync::{Arc, OnceLock, atomic::AtomicBool},
};
#[cfg(feature = "wgpu")]
use wgpu::{Device, Queue, TextureView};
#[cfg(all(feature = "wgpu", feature = "wgpu-profiler"))]
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings};

/// Represents the anti-aliasing method to use during a render pass.
///
/// Can be configured for a render operation by setting [`RenderParams::antialiasing_method`].
/// Each value of this can only be used if the corresponding field on [`AaSupport`] was used.
///
/// This can be converted into an `AaSupport` using [`Iterator::collect`],
/// as `AaSupport` implements `FromIterator`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AaConfig {
    /// Area anti-aliasing, where the alpha value for a pixel is computed from integrating
    /// the winding number over its square area.
    ///
    /// This technique produces very accurate values when the shape has winding number of 0 or 1
    /// everywhere, but can result in conflation artifacts otherwise.
    /// It generally has better performance than the multi-sampling methods.
    ///
    /// Can only be used if [enabled][AaSupport::area] for the `Renderer`.
    Area,
    /// 8x Multisampling
    ///
    /// Can only be used if [enabled][AaSupport::msaa8] for the `Renderer`.
    Msaa8,
    /// 16x Multisampling
    ///
    /// Can only be used if [enabled][AaSupport::msaa16] for the `Renderer`.
    Msaa16,
}

/// Represents the set of anti-aliasing configurations to enable during pipeline creation.
///
/// This is configured at `Renderer` creation time ([`Renderer::new`]) by setting
/// [`RendererOptions::antialiasing_support`].
///
/// This can be created from a set of `AaConfig` using [`Iterator::collect`],
/// as `AaSupport` implements `FromIterator`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AaSupport {
    /// Support [`AaConfig::Area`].
    pub area: bool,
    /// Support [`AaConfig::Msaa8`].
    pub msaa8: bool,
    /// Support [`AaConfig::Msaa16`].
    pub msaa16: bool,
}

impl AaSupport {
    /// Support every anti-aliasing method.
    ///
    /// This might increase startup time, as more shader variations must be compiled.
    pub fn all() -> Self {
        Self {
            area: true,
            msaa8: true,
            msaa16: true,
        }
    }

    /// Support only [`AaConfig::Area`].
    ///
    /// This should be the default choice for most users.
    pub fn area_only() -> Self {
        Self {
            area: true,
            msaa8: false,
            msaa16: false,
        }
    }
}

impl FromIterator<AaConfig> for AaSupport {
    fn from_iter<T: IntoIterator<Item = AaConfig>>(iter: T) -> Self {
        let mut result = Self {
            area: false,
            msaa8: false,
            msaa16: false,
        };
        for config in iter {
            match config {
                AaConfig::Area => result.area = true,
                AaConfig::Msaa8 => result.msaa8 = true,
                AaConfig::Msaa16 => result.msaa16 = true,
            }
        }
        result
    }
}

/// Errors that can occur in Vello.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// There is no available device with the features required by Vello.
    #[cfg(feature = "wgpu")]
    #[error("Couldn't find suitable device")]
    NoCompatibleDevice,
    /// Failed to create surface.
    /// See [`wgpu::CreateSurfaceError`] for more information.
    #[cfg(feature = "wgpu")]
    #[error("Couldn't create wgpu surface")]
    WgpuCreateSurfaceError(#[from] wgpu::CreateSurfaceError),
    /// Surface doesn't support the required texture formats.
    /// Make sure that you have a surface which provides one of
    /// [`TextureFormat::Rgba8Unorm`][wgpu::TextureFormat::Rgba8Unorm]
    /// or [`TextureFormat::Bgra8Unorm`][wgpu::TextureFormat::Bgra8Unorm] as texture formats.
    // TODO: Why does this restriction exist?
    #[cfg(feature = "wgpu")]
    #[error("Couldn't find `Rgba8Unorm` or `Bgra8Unorm` texture formats for surface")]
    UnsupportedSurfaceFormat,

    /// Used a buffer inside a recording while it was not available.
    /// Check if you have created it and not freed before its last usage.
    #[cfg(feature = "wgpu")]
    #[error("Buffer '{0}' is not available but used for {1}")]
    UnavailableBufferUsed(&'static str, &'static str),
    /// Failed to async map a buffer.
    /// See [`wgpu::BufferAsyncError`] for more information.
    #[cfg(feature = "wgpu")]
    #[error("Failed to async map a buffer")]
    BufferAsyncError(#[from] wgpu::BufferAsyncError),
    /// Failed to download an internal buffer for debug visualization.
    #[cfg(feature = "wgpu")]
    #[cfg(feature = "debug_layers")]
    #[error("Failed to download internal buffer '{0}' for visualization")]
    DownloadError(&'static str),

    #[cfg(feature = "wgpu")]
    #[error("wgpu Error from scope")]
    WgpuErrorFromScope(#[from] wgpu::Error),

    /// Failed to create [`GpuProfiler`].
    /// See [`wgpu_profiler::CreationError`] for more information.
    #[cfg(feature = "wgpu-profiler")]
    #[error("Couldn't create wgpu profiler")]
    #[doc(hidden)] // End-users of Vello should not have `wgpu-profiler` enabled.
    ProfilerCreationError(#[from] wgpu_profiler::CreationError),

    /// Failed to compile the shaders.
    #[cfg(feature = "hot_reload")]
    #[error("Failed to compile shaders:\n{0}")]
    #[doc(hidden)] // End-users of Vello should not have `hot_reload` enabled.
    ShaderCompilation(#[from] vello_shaders::compile::ErrorVec),
}

#[cfg_attr(
    not(feature = "wgpu"),
    expect(dead_code, reason = "this can be unused when wgpu feature is not used")
)]
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

/// Renders a scene into a texture or surface.
///
/// Currently, each renderer only supports a single surface format, if it
/// supports drawing to surfaces at all.
/// This is an assumption which is known to be limiting, and is planned to change.
#[cfg(feature = "wgpu")]
pub struct Renderer {
    #[cfg_attr(
        not(feature = "hot_reload"),
        expect(
            dead_code,
            reason = "Options are only used to reinitialise on a hot reload"
        )
    )]
    options: RendererOptions,
    engine: WgpuEngine,
    resolver: Resolver,
    image_atlas: Option<recording::ImageProxy>,
    shaders: FullShaders,
    /// The sizes of the bump-allocated buffers while they grow on demand; see
    /// [`Renderer::set_bump_sizes`].
    bump_sizes: Option<BumpAllocators>,
    /// Downloads of bump allocators not read yet, each with the outcome of its mapping once the
    /// GPU has answered.
    bump_reads: Vec<(BufferProxy, Arc<OnceLock<bool>>)>,
    #[cfg(feature = "debug_layers")]
    debug: debug::DebugRenderer,
    #[cfg(feature = "wgpu-profiler")]
    #[doc(hidden)] // End-users of Vello should not have `wgpu-profiler` enabled.
    /// The profiler used with events for this renderer. This is *not* treated as public API.
    pub profiler: GpuProfiler,
    #[cfg(feature = "wgpu-profiler")]
    #[doc(hidden)] // End-users of Vello should not have `wgpu-profiler` enabled.
    /// The results from profiling. This is *not* treated as public API.
    pub profile_result: Option<Vec<wgpu_profiler::GpuTimerQueryResult>>,
}
// This is not `Send` (or `Sync`) on WebAssembly as the
// underlying wgpu types are not. This can be enabled with the
// `fragile-send-sync-non-atomic-wasm` feature in wgpu.
// See https://github.com/gfx-rs/wgpu/discussions/4127 for
// further discussion of this topic.
#[cfg(all(feature = "wgpu", not(target_arch = "wasm32")))]
static_assertions::assert_impl_all!(Renderer: Send);

/// Parameters used in a single render that are configurable by the client.
///
/// These are used in [`Renderer::render_to_texture`].
pub struct RenderParams {
    /// The background color applied to the target. This value is only applicable to the full
    /// pipeline.
    pub base_color: peniko::Color,

    /// Dimensions of the rasterization target
    pub width: u32,
    pub height: u32,

    /// The anti-aliasing algorithm. The selected algorithm must have been initialized while
    /// constructing the `Renderer`.
    pub antialiasing_method: AaConfig,
}

#[cfg(feature = "wgpu")]
/// Options which are set at renderer creation time, used in [`Renderer::new`].
pub struct RendererOptions {
    /// If true, run all stages up to fine rasterization on the CPU.
    ///
    /// This is not a recommended configuration as it is expected to have poor performance,
    /// but it can be useful for debugging.
    // TODO: Consider evolving this so that the CPU stages can be configured dynamically via
    // `RenderParams`.
    pub use_cpu: bool,

    /// Represents the enabled set of AA configurations. This will be used to determine which
    /// pipeline permutations should be compiled at startup.
    ///
    /// By default this will be all modes, to support the widest range of.
    /// It is recommended that most users configure this.
    pub antialiasing_support: AaSupport,

    /// How many threads to use for initialisation of shaders.
    ///
    /// Use `Some(1)` to use a single thread. This is recommended when on macOS
    /// (see <https://github.com/bevyengine/bevy/pull/10812#discussion_r1496138004>)
    ///
    /// Set to `None` to use a heuristic which will use many but not all threads
    ///
    /// Has no effect on WebAssembly
    ///
    /// Will default to `None` on most platforms, `Some(1)` on macOS.
    pub num_init_threads: Option<NonZeroUsize>,

    /// The pipeline cache to use when creating the shaders.
    ///
    /// For much more discussion of expected usage patterns, see the documentation on that type.
    pub pipeline_cache: Option<wgpu::PipelineCache>,
}

#[cfg(feature = "wgpu")]
impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            use_cpu: false,
            antialiasing_support: AaSupport::all(),
            #[cfg(target_os = "macos")]
            num_init_threads: NonZeroUsize::new(1),
            #[cfg(not(target_os = "macos"))]
            num_init_threads: None,
            pipeline_cache: None,
        }
    }
}

#[cfg(feature = "wgpu")]
struct RenderResult {
    bump: Option<BumpAllocators>,
    #[cfg(feature = "debug_layers")]
    captured: Option<render::CapturedBuffers>,
}

/// What [`Renderer::poll_bump`] found.
#[cfg(feature = "wgpu")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BumpPoll {
    /// The bump allocators of a render have not been answered yet: poll again soon.
    pub pending: bool,
    /// The buffers grew: every render made before this poll may have lost its content and
    /// should be made again.
    pub grown: bool,
}

/// Grows each size in `sizes` that `bump`, read back after a render, shows too small: to the
/// next power of two at or above twice the need, at most 16 times the fixed size of
/// [`BufferSizes::new`](vello_encoding::BufferSizes::new) and what the device can bind in
/// `max_binding` bytes. Never shrinks. Returns whether any size grew.
#[cfg(feature = "wgpu")]
fn grow_bump_sizes(sizes: &mut BumpAllocators, bump: &BumpAllocators, max_binding: u64) -> bool {
    static HAS_WARNED: AtomicBool = AtomicBool::new(false);
    let words = max_binding / size_of::<u32>() as u64;
    // `bin_data` and `ptcl` hold a part of their own before the dynamic one: they get half the
    // binding to leave room for it.
    let fields: [(&mut u32, u32, u64, u64); 7] = [
        (&mut sizes.lines, bump.lines, 1 << 21, max_binding / size_of::<LineSoup>() as u64),
        (&mut sizes.binning, bump.binning, 1 << 18, words / 2),
        (&mut sizes.tile, bump.tile, 1 << 21, max_binding / size_of::<Tile>() as u64),
        (
            &mut sizes.seg_counts,
            bump.seg_counts,
            1 << 21,
            max_binding / size_of::<SegmentCount>() as u64,
        ),
        (
            &mut sizes.segments,
            bump.segments,
            1 << 21,
            max_binding / size_of::<PathSegment>() as u64,
        ),
        (&mut sizes.blend, bump.blend, 1 << 20, words),
        (&mut sizes.ptcl, bump.ptcl, 1 << 23, words / 2),
    ];
    let mut grown = false;
    for (size, need, fixed, bindable) in fields {
        if need <= *size {
            continue;
        }
        let max = (16 * fixed).min(bindable);
        let new = (2 * u64::from(need)).next_power_of_two().min(max) as u32;
        if new > *size {
            *size = new;
            grown = true;
        } else if !HAS_WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            log::warn!(
                "A scene needs more than the largest bump-allocated buffer Vello allows ({need} > {size}); it is not drawn"
            );
        }
    }
    grown
}

#[cfg(feature = "wgpu")]
impl Renderer {
    /// Creates a new renderer for the specified device.
    pub fn new(device: &Device, options: RendererOptions) -> Result<Self> {
        let mut engine = WgpuEngine::new(options.use_cpu, options.pipeline_cache.clone());
        // If we are running in parallel (i.e. the number of threads is not 1)
        if options.num_init_threads != NonZeroUsize::new(1) {
            #[cfg(not(target_arch = "wasm32"))]
            engine.use_parallel_initialisation();
        }
        let shaders = shaders::full_shaders(device, &mut engine, &options)?;
        #[cfg(not(target_arch = "wasm32"))]
        engine.build_shaders_if_needed(device, options.num_init_threads);
        #[cfg(feature = "debug_layers")]
        let debug = debug::DebugRenderer::new(device, wgpu::TextureFormat::Rgba8Unorm, &mut engine);

        Ok(Self {
            options,
            engine,
            resolver: Resolver::new(),
            image_atlas: None,
            shaders,
            #[cfg(feature = "debug_layers")]
            debug,
            #[cfg(feature = "wgpu-profiler")]
            profiler: GpuProfiler::new(device, GpuProfilerSettings::default())?,
            #[cfg(feature = "wgpu-profiler")]
            profile_result: None,
            bump_sizes: None,
            bump_reads: Vec::new(),
        })
    }

    /// Renders a scene to the target texture.
    ///
    /// The texture is assumed to be of the specified dimensions and have been created with
    /// the [`wgpu::TextureFormat::Rgba8Unorm`] format and the [`wgpu::TextureUsages::STORAGE_BINDING`]
    /// flag set.
    ///
    /// If you want to render Vello content to a surface (such as in a UI toolkit), you have two options:
    /// 1) Render to an intermediate texture, which is the same size as the surface.
    ///    You would then use [`TextureBlitter`][wgpu::util::TextureBlitter] to blit the rendered result from
    ///    that texture to the surface.
    ///    This pattern is supported by the [`util`] module.
    /// 2) Call `render_to_texture` directly on the [`SurfaceTexture`][wgpu::SurfaceTexture]'s texture, if
    ///    it has the right usages. This should generally be avoided, as some GPUs assume that you will not
    ///    be rendering to the surface using a compute pipeline, and optimise accordingly.
    pub fn render_to_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        scene: &Scene,
        texture: &TextureView,
        params: &RenderParams,
    ) -> Result<()> {
        let bump_sizes = self.bump_sizes.filter(|_| !self.engine.use_cpu);
        let (recording, target, bump_buf) = render::render_full(
            scene,
            &mut self.resolver,
            &self.shaders,
            &mut self.image_atlas,
            params,
            bump_sizes,
        );
        let external_resources = [ExternalResource::Image(
            *target.as_image().unwrap(),
            texture,
        )];
        self.engine.run_recording(
            device,
            queue,
            &recording,
            &external_resources,
            "render_to_texture",
            #[cfg(feature = "wgpu-profiler")]
            &mut self.profiler,
        )?;
        if let Some(bump_buf) = bump_buf
            && let Some(buf) = self.engine.get_download(bump_buf)
        {
            let mapped = Arc::new(OnceLock::new());
            let answer = mapped.clone();
            buf.slice(..).map_async(wgpu::MapMode::Read, move |result| {
                let _ = answer.set(result.is_ok());
            });
            self.bump_reads.push((bump_buf, mapped));
        }
        // N.B. This is horrible; this integration of wgpu-profiler really needs some work...
        #[cfg(feature = "wgpu-profiler")]
        {
            self.profiler.end_frame().unwrap();
            if let Some(result) = self
                .profiler
                .process_finished_frame(queue.get_timestamp_period())
            {
                self.profile_result = Some(result);
            }
        }

        Ok(())
    }

    /// Starting sizes for [`set_bump_sizes`](Self::set_bump_sizes): about 8 MB for the seven
    /// buffers, plus 64 words per tile of the target in `ptcl` and the draw info in `bin_data`.
    /// The fixed sizes take about 165 MB.
    pub const SMALL_BUMP_SIZES: BumpAllocators = BumpAllocators {
        failed: 0,
        binning: 1 << 16,
        ptcl: 1 << 18,
        tile: 1 << 17,
        seg_counts: 1 << 17,
        segments: 1 << 17,
        blend: 1 << 16,
        lines: 1 << 16,
    };

    /// Sizes the seven bump-allocated work buffers (lines, binning, tiles, segment counts,
    /// segments, blend spill, per-tile command lists) from `sizes` and lets them grow; `None`,
    /// the default, keeps the fixed sizes, which are large and never grow.
    ///
    /// `sizes` are in the units of the GPU's counters, as `RenderConfig::set_bump_sizes` says.
    /// While they are set, each [`render_to_texture`](Self::render_to_texture) also downloads
    /// the bump allocators, without waiting; [`poll_bump`](Self::poll_bump) reads them and grows
    /// the buffers a render overflowed. A render that overflowed draws nothing and must be made
    /// again. No effect with [`RendererOptions::use_cpu`].
    pub fn set_bump_sizes(&mut self, sizes: Option<BumpAllocators>) {
        self.bump_sizes = sizes;
        self.engine.clear_pool();
    }

    /// Reads, without waiting, the bump allocators of the renders the GPU has finished, and
    /// grows the buffers they show too small (see [`set_bump_sizes`](Self::set_bump_sizes)).
    /// A stage that overflows skips the later stages, which then report no need, so a scene may
    /// take a few rounds to fit. The pool of free buffers is emptied when the sizes grow.
    pub fn poll_bump(&mut self, device: &Device) -> BumpPoll {
        let mut poll = BumpPoll::default();
        if self.bump_reads.is_empty() {
            return poll;
        }
        // Runs the callbacks of the mappings that are done; never blocks.
        let _ = device.poll(wgpu::PollType::Poll);
        let max_binding = device.limits().max_storage_buffer_binding_size;
        self.bump_reads.retain(|(proxy, mapped)| {
            let Some(&ok) = mapped.get() else {
                poll.pending = true;
                return true;
            };
            let bump = self
                .engine
                .get_download(*proxy)
                .filter(|_| ok)
                .and_then(|buf| {
                    let bump = buf
                        .slice(..)
                        .get_mapped_range()
                        .ok()
                        .map(|view| bytemuck::pod_read_unaligned::<BumpAllocators>(&view));
                    buf.unmap();
                    bump
                });
            self.engine.free_download(*proxy);
            if let (Some(bump), Some(sizes)) = (bump, &mut self.bump_sizes) {
                poll.grown |= grow_bump_sizes(sizes, &bump, max_binding);
            }
            false
        });
        if poll.grown {
            self.engine.clear_pool();
        }
        poll
    }

    /// Overwrite `image` with `texture`.
    ///
    /// Most users should prefer [`register_texture`](Self::register_texture), which
    /// ergonomically encapulates these requirements.
    ///
    /// `texture` must have the [`wgpu::TextureFormat::Rgba8Unorm`] format and
    /// the [`wgpu::TextureUsages::COPY_SRC`] flag set. The `Rgba8UnormSrgb` format
    /// might also be supported, but this is not tested.
    ///
    /// The given `Texture`'s data will be copied into the slot in Vello's image
    /// atlas where the image would be placed each frame.
    /// This has the effect that wherever you request the image to be drawn (e.g.
    /// through [`Scene::draw_image`]), the data from the texture will be used instead.
    ///
    /// Correct behaviour is not guaranteed if the texture does not have the same
    /// dimensions as the image, nor if an image which uses the same [blob] but different
    /// dimensions would be rendered.
    ///
    /// [blob]: peniko::ImageData::data
    pub fn override_image(
        &mut self,
        image: &ImageData,
        texture: Option<wgpu::TexelCopyTextureInfoBase<wgpu::Texture>>,
    ) -> Option<wgpu::TexelCopyTextureInfoBase<wgpu::Texture>> {
        self.resolver.mark_image_dirty(image);
        match texture {
            Some(texture) => self.engine.image_overrides.insert(image.data.id(), texture),
            None => self.engine.image_overrides.remove(&image.data.id()),
        }
    }

    /// Marks `image` as dirty in the atlas cache, so its override texture will be recopied on
    /// the next render that uses it.
    ///
    /// Call this for any `image` whose existing override texture has changed since the last render.
    /// Otherwise, stale image data from the atlas may be used.
    pub fn mark_override_image_dirty(&mut self, image: &ImageData) {
        self.resolver.mark_image_dirty(image);
    }

    /// Register a [`wgpu::Texture`] with Vello, to allow drawing GPU-resident data.
    ///
    /// The returned `Image` can be used in [`Scene`]s (only those rendered with this `Renderer`)
    /// Rendering Scenes which use this `Image` with other `Renderer`s will panic.
    ///  
    /// `texture` must have the [`wgpu::TextureFormat::Rgba8Unorm`] format and
    /// the [`wgpu::TextureUsages::COPY_SRC`] flag set. This is because the data will
    /// be copied into Vello's image atlas at the start of each frame.
    /// The `Rgba8UnormSrgb` format might also be supported, but this is not tested.
    /// The texture is assumed to have unpremultiplied alpha.
    ///
    /// This is a utility wrapper around [`override_image`](Self::override_image).
    /// For greater control, use that method.
    ///
    /// If the texture is no longer active then it should be unregistered using [`unregister_texture`](Self::unregister_texture)
    pub fn register_texture(&mut self, texture: wgpu::Texture) -> ImageData {
        // Create a fake, empty blob which will be used to back the returned image
        // This image data will never be read by Vello, due to being added to
        // image_overrides, below.
        let fake_blob = peniko::Blob::new(std::sync::Arc::new(&[]));

        let image = ImageData {
            data: fake_blob,
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: texture.width(),
            height: texture.height(),
        };

        // For this utility API, we take the full texture and use the base layer and mip level
        let texture_base = wgpu::TexelCopyTextureInfoBase {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };

        // We overwrite any attempt to use the fake blob, instead reading from the texture.
        self.override_image(&image, Some(texture_base));

        image
    }

    /// Unregister a [`wgpu::Texture`] that was registered with [`register_texture`](Self::register_texture).
    pub fn unregister_texture(&mut self, handle: ImageData) {
        self.override_image(&handle, None);
    }

    /// Reload the shaders. This should only be used during `vello` development
    #[cfg(feature = "hot_reload")]
    #[doc(hidden)] // End-users of Vello should not have `hot_reload` enabled.
    pub async fn reload_shaders(&mut self, device: &Device) -> Result<(), Error> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut engine = WgpuEngine::new(self.options.use_cpu, self.options.pipeline_cache.clone());
        // We choose not to initialise these shaders in parallel, to ensure the error scope works correctly
        let shaders = shaders::full_shaders(device, &mut engine, &self.options)?;
        #[cfg(feature = "debug_layers")]
        let debug = debug::DebugRenderer::new(device, wgpu::TextureFormat::Rgba8Unorm, &mut engine);
        let error = scope.pop().await;
        if let Some(error) = error {
            return Err(error.into());
        }
        self.engine = engine;
        self.image_atlas = None;
        self.shaders = shaders;
        #[cfg(feature = "debug_layers")]
        {
            self.debug = debug;
        }
        Ok(())
    }

    /// Renders a scene to the target texture using an async pipeline.
    ///
    /// Almost all consumers should prefer [`Self::render_to_texture`].
    ///
    /// The return value is the value of the `BumpAllocators` in this rendering, which is currently used
    /// for debug output.
    ///
    /// This return type is not stable, and will likely be changed when a more principled way to access
    /// relevant statistics is implemented
    #[cfg_attr(docsrs, doc(hidden))]
    #[deprecated(
        note = "render_to_texture should be preferred, as the _async version has no stability guarantees"
    )]
    pub async fn render_to_texture_async(
        &mut self,
        device: &Device,
        queue: &Queue,
        scene: &Scene,
        texture: &TextureView,
        params: &RenderParams,
        debug_layers: DebugLayers,
    ) -> Result<Option<BumpAllocators>> {
        if cfg!(not(feature = "debug_layers")) && !debug_layers.is_empty() {
            static HAS_WARNED: AtomicBool = AtomicBool::new(false);
            if !HAS_WARNED.swap(true, std::sync::atomic::Ordering::Release) {
                log::warn!(
                    "Requested debug layers {debug_layers:?} but `debug_layers` feature is not enabled"
                );
            }
        }

        let result = self
            .render_to_texture_async_internal(device, queue, scene, texture, params)
            .await?;

        #[cfg(feature = "debug_layers")]
        {
            let mut recording = Recording::default();
            let target_proxy = recording::ImageProxy::new(
                params.width,
                params.height,
                recording::ImageFormat::Rgba8,
            );
            if let Some(captured) = result.captured {
                let bump = result.bump.as_ref().unwrap();
                // TODO: We could avoid this download if `DebugLayers::VALIDATION` is unset.
                let downloads = DebugDownloads::map(&self.engine, &captured, bump).await?;
                self.debug.render(
                    &mut recording,
                    target_proxy,
                    &captured,
                    bump,
                    params,
                    &downloads,
                    debug_layers,
                );

                // TODO: this sucks. better to release everything in a helper
                // TODO: it would be much better to have a way to safely destroy a buffer.
                self.engine.free_download(captured.lines);
                captured.release_buffers(&mut recording);
            }
            let external_resources = [ExternalResource::Image(target_proxy, texture)];
            self.engine.run_recording(
                device,
                queue,
                &recording,
                &external_resources,
                "render_to_texture_async debug layers",
                #[cfg(feature = "wgpu-profiler")]
                &mut self.profiler,
            )?;
        }

        #[cfg(feature = "wgpu-profiler")]
        {
            self.profiler.end_frame().unwrap();
            if let Some(result) = self
                .profiler
                .process_finished_frame(queue.get_timestamp_period())
            {
                self.profile_result = Some(result);
            }
        }

        Ok(result.bump)
    }

    async fn render_to_texture_async_internal(
        &mut self,
        device: &Device,
        queue: &Queue,
        scene: &Scene,
        texture: &TextureView,
        params: &RenderParams,
    ) -> Result<RenderResult> {
        let mut render = Render::new();
        let encoding = scene.encoding();
        // TODO: turn this on; the download feature interacts with CPU dispatch.
        // Currently this is always enabled when the `debug_layers` setting is enabled as the bump
        // counts are used for debug visualiation.
        let robust = cfg!(feature = "debug_layers");
        let recording = render.render_encoding_coarse(
            encoding,
            &mut self.resolver,
            &self.shaders,
            &mut self.image_atlas,
            params,
            robust,
        );
        let target = render.out_image();
        let bump_buf = render.bump_buf();
        #[cfg(feature = "debug_layers")]
        let captured = render.take_captured_buffers();
        self.engine.run_recording(
            device,
            queue,
            &recording,
            &[],
            "t_async_coarse",
            #[cfg(feature = "wgpu-profiler")]
            &mut self.profiler,
        )?;

        let mut bump: Option<BumpAllocators> = None;
        if let Some(bump_buf) = self.engine.get_download(bump_buf) {
            let buf_slice = bump_buf.slice(..);
            let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
            buf_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
            receiver.receive().await.expect("channel was closed")?;
            let mapped = buf_slice.get_mapped_range().unwrap();
            bump = Some(bytemuck::pod_read_unaligned(&mapped));
        }
        // TODO: apply logic to determine whether we need to rerun coarse, and also
        // allocate the blend stack as needed.
        self.engine.free_download(bump_buf);
        // Maybe clear to reuse allocation?
        let mut recording = Recording::default();
        render.record_fine(&self.shaders, &mut recording);
        let external_resources = [ExternalResource::Image(target, texture)];
        self.engine.run_recording(
            device,
            queue,
            &recording,
            &external_resources,
            "t_async_fine",
            #[cfg(feature = "wgpu-profiler")]
            &mut self.profiler,
        )?;
        Ok(RenderResult {
            bump,
            #[cfg(feature = "debug_layers")]
            captured,
        })
    }
}
#[cfg(all(feature = "debug_layers", feature = "wgpu"))]
pub(crate) struct DebugDownloads<'a> {
    pub lines: wgpu::BufferSlice<'a>,
}

#[cfg(all(feature = "debug_layers", feature = "wgpu"))]
impl<'a> DebugDownloads<'a> {
    pub async fn map(
        engine: &'a WgpuEngine,
        captured: &render::CapturedBuffers,
        bump: &BumpAllocators,
    ) -> Result<DebugDownloads<'a>> {
        use vello_encoding::LineSoup;

        let Some(lines_buf) = engine.get_download(captured.lines) else {
            return Err(Error::DownloadError("linesoup"));
        };

        let lines = lines_buf.slice(..bump.lines as u64 * size_of::<LineSoup>() as u64);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        lines.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
        receiver.receive().await.expect("channel was closed")?;
        Ok(Self { lines })
    }
}

#[cfg(all(test, feature = "wgpu"))]
mod tests {
    use super::{BumpAllocators, Renderer, grow_bump_sizes};

    #[test]
    fn bump_sizes_grow_to_twice_the_need_up_to_what_binds() {
        const BINDING: u64 = 128 << 20;
        let mut sizes = Renderer::SMALL_BUMP_SIZES;
        let need = BumpAllocators {
            lines: (1 << 16) + 1,
            ..Default::default()
        };
        assert!(grow_bump_sizes(&mut sizes, &need, BINDING));
        assert_eq!(sizes.lines, 1 << 18);
        assert_eq!(sizes.ptcl, Renderer::SMALL_BUMP_SIZES.ptcl);
        assert!(!grow_bump_sizes(&mut sizes, &need, BINDING));
        // 24-byte lines: 5,592,405 fit in 128 MiB, below 16 times the fixed 1 << 21.
        let huge = BumpAllocators {
            lines: u32::MAX / 2,
            ..Default::default()
        };
        assert!(grow_bump_sizes(&mut sizes, &huge, BINDING));
        assert_eq!(sizes.lines, (BINDING / 24) as u32);
        assert!(!grow_bump_sizes(&mut sizes, &huge, BINDING));
    }
}
