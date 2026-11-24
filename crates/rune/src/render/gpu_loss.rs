//! Device loss: the callback that tells Rune a device went away, and what a surface does about
//! it (`window/host/create.rs::Host::reattach`). wgpu reports a dead device only through
//! `Device::set_device_lost_callback`; `Surface::configure`'s `DeviceLost` is dropped, so a lost
//! device would otherwise surface as a panic on the surface's next drawable.
//!
//! The native Metal backend (`gpu/metal/device.rs`) reports both failed command buffers and
//! `Device::destroy` through `set_device_lost_callback`, setting `is_lost`.

use super::Gpu;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

/// The distinct messages `Gpu::made` has printed, bounded: an error inside the frame would
/// otherwise print on every frame.
#[cfg(not(target_arch = "wasm32"))]
const PRINTED: usize = 16;

impl Gpu {
    /// A GPU over a device made elsewhere (`device_on`, a native backend, an example): Rune
    /// watches it for loss, and on a surface device logs its uncaptured errors rather than
    /// letting wgpu's default handler panic the app. That handler replaces any handler set on
    /// the device before it was handed over.
    pub fn from_device(instance: wgpu::Instance, adapter: wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue) -> Gpu {
        Self::made(instance, adapter, device, queue, true)
    }

    /// `from_device`, saying whether this device draws surfaces (`Host::try_create`,
    /// `Overlay::attach`): only those log uncaptured errors. A headless device is usually a
    /// test's or an offscreen render's, where a validation error should still fail loudly, and
    /// under `cfg(test)` nothing logs so Rune's own tests see wgpu's panic.
    pub(crate) fn made(instance: wgpu::Instance, adapter: wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue,
        draws_surfaces: bool) -> Gpu {
        let removal = rune_gpu::health::Removal::watch(&device);
        let gpu = Gpu {
            instance, adapter, device, queue, lost: Arc::new(AtomicBool::new(false)), removal,
            #[cfg(not(target_arch = "wasm32"))]
            uncaptured: uncaptured_handler(draws_surfaces && !cfg!(test)),
        };
        gpu.watch_loss();
        // On wasm32, `platform::web::health::watch` owns uncaptured error logging for both WebGPU
        // (through JS, avoiding wgpu's panic on `GPUInternalError`) and WebGL2 (to `console.error`,
        // where `eprintln!` is silent).
        #[cfg(not(target_arch = "wasm32"))]
        if draws_surfaces && !cfg!(test) { gpu.device.on_uncaptured_error(gpu.uncaptured.clone()); }
        #[cfg(target_arch = "wasm32")]
        let _ = draws_surfaces;
        gpu
    }

    fn watch_loss(&self) {
        let lost = self.lost.clone();
        self.device.set_device_lost_callback(move |reason, message| {
            // Before anything reads it: a lost device's surface can't be configured, and wgpu
            // reports that failure nowhere (`is_lost`).
            lost.store(true, Ordering::Release);
            // No panic here, in tests either: wgpu calls this inside its own work (a submission,
            // a queue's drop) and a panic there could abort the process. A test fails at its
            // next wait instead (`rune_gpu::health::poll` returns `Lost`).
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("rune: the GPU device was lost ({reason:?}): {message}");
            #[cfg(target_arch = "wasm32")]
            let _ = (reason, message);
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn restore_device_callbacks(&self) {
        self.watch_loss();
        self.device.on_uncaptured_error(self.uncaptured.clone());
    }

    /// Whether the device went away: a driver reset, a GPU removed, a resume from Modern Standby.
    /// Nothing can be drawn with it, and its surfaces can't be configured, so its owner makes a
    /// new GPU and surface instead (`Host::reattach`).
    /// On D3D12 the device is asked too: wgpu hears of a removal only at the next submission.
    pub fn is_lost(&self) -> bool { self.lost.load(Ordering::Acquire) || self.removal.removed() }
}

/// What the surface error handler has printed: each distinct message once, up to `PRINTED`, then one
/// line saying the rest are silent.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Printed {
    seen: Vec<String>,
    silenced: bool,
}

/// Prints each uncaptured error once, up to `PRINTED` distinct messages, then one line saying
/// further ones are not shown. A scoped error (a `push_error_scope`, as `window::configure`
/// uses) still goes to its scope first; this is the fallback for errors nothing else caught.
#[cfg(not(target_arch = "wasm32"))]
fn uncaptured_handler(log: bool) -> Arc<dyn wgpu::UncapturedErrorHandler> {
    if !log {
        // wgpu exposes no setter for its original default handler. Headless validation is fatal.
        return Arc::new(|error: wgpu::Error| panic!("wgpu error: {error}"));
    }
    let printed = Mutex::new(Printed::default());
    Arc::new(move |error: wgpu::Error| {
        let message = error.to_string();
        let line = {
            let mut printed = printed.lock().unwrap_or_else(|e| e.into_inner());
            if printed.silenced || printed.seen.contains(&message) {
                None
            } else if printed.seen.len() < PRINTED {
                printed.seen.push(message.clone());
                Some(format!("rune: a GPU error went uncaptured: {message}"))
            } else {
                printed.silenced = true;
                Some(format!("rune: {PRINTED} distinct GPU errors went uncaptured; further uncaptured GPU errors are not shown"))
            }
        };
        if let Some(line) = line { eprintln!("{line}"); }
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "gpu_loss_tests.rs"]
mod tests;
