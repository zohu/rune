use super::Gpu;
use std::sync::Arc;

fn invalid_buffer(device: &wgpu::Device) {
    let _ = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shared callback validation"), size: 16, usage: wgpu::BufferUsages::empty(), mapped_at_creation: false,
    });
}

// Replacing the handlers must not detach the device from the loss flag its windows already share.
#[test]
fn restoring_callbacks_reports_destroy_on_the_original_device() {
    let gpu = Gpu::headless_wgpu();
    let other = Gpu::headless_wgpu();
    gpu.device.set_device_lost_callback(|_, _| {});
    gpu.restore_device_callbacks();
    gpu.device.destroy();
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    assert!(gpu.is_lost());
    assert!(!other.is_lost());
}

#[test]
#[should_panic(expected = "wgpu error:")]
fn restoring_headless_callbacks_makes_unscoped_validation_fatal() {
    let gpu = Gpu::headless_wgpu();
    gpu.device.on_uncaptured_error(Arc::new(|_| {}));
    gpu.restore_device_callbacks();
    invalid_buffer(&gpu.device);
}

#[test]
fn restored_surface_handler_logs_unscoped_validation() {
    let mut gpu = Gpu::headless_wgpu();
    // Rune's unit tests normally panic even on surface devices.
    gpu.uncaptured = super::uncaptured_handler(true);
    gpu.restore_device_callbacks();
    invalid_buffer(&gpu.device);
    gpu.device.on_uncaptured_error(Arc::new(|_| panic!("external handler still installed")));
    gpu.restore_device_callbacks();
    invalid_buffer(&gpu.device);
}
