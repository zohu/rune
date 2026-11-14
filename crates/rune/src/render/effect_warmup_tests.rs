use super::*;
fn rect(bounds: [f32; 4], color: [f32; 4]) -> Instance {
    Instance {
        rect: bounds,
        fill: color,
        clip: NO_CLIP,
        params: [0.0, KIND_RECT, 0.0, 1.0],
        ..Instance::zeroed()
    }
}

fn pixels(gpu: &Gpu, renderer: &Renderer, target: &Offscreen) -> Vec<u8> {
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    renderer.draw(&mut encoder, &target.view, [0.0; 4]);
    target.copy(&mut encoder);
    renderer.submit(&gpu.queue, encoder);
    target.read(&gpu.device)
}

fn finish(renderer: &mut Renderer, device: &wgpu::Device) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !matches!(renderer.effect_warmup, State::Done { .. }) {
        renderer.prepare_effect_pipelines(device);
        assert!(
            std::time::Instant::now() < deadline,
            "effect warmup did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn warmed_effects_keep_lazy_pixels_and_allocate_no_layer_targets() {
    let gpu = Gpu::headless();
    let mut lazy = Renderer::new(&gpu.device, OFFSCREEN_FORMAT);
    let mut warm = Renderer::new(&gpu.device, OFFSCREEN_FORMAT);
    finish(&mut warm, &gpu.device);
    assert_eq!(warm.layer_stats().bytes, 0);
    let content = [
        Instance::begin_layer(
            [8.0, 8.0, 24.0, 24.0],
            NO_CLIP,
            LayerSpec {
                blur: 2.0,
                opacity: 0.6,
                ..Default::default()
            },
        ),
        rect([8.0, 8.0, 24.0, 24.0], [1.0, 0.0, 0.0, 1.0]),
        Instance::end_layer(),
    ];
    let mut atlas = crate::text::TextSystem::default().atlas;
    let output = Offscreen::new(&gpu.device, 48, 48);
    lazy.prepare(&gpu.device, &gpu.queue, &mut atlas, &content, (48, 48));
    warm.prepare(&gpu.device, &gpu.queue, &mut atlas, &content, (48, 48));
    assert_eq!(pixels(&gpu, &lazy, &output), pixels(&gpu, &warm, &output));
}

#[test]
fn changing_target_while_warming_keeps_the_new_format_drawable() {
    let gpu = Gpu::headless();
    let mut renderer = Renderer::new(&gpu.device, OFFSCREEN_FORMAT);
    renderer.prepare_effect_pipelines(&gpu.device);
    renderer.prepare_effect_pipelines(&gpu.device);
    renderer.set_format(&gpu.device, wgpu::TextureFormat::Bgra8Unorm);
    finish(&mut renderer, &gpu.device);
    let content = [rect([0.0, 0.0, 16.0, 16.0], [1.0; 4])];
    let mut atlas = crate::text::TextSystem::default().atlas;
    renderer.prepare(&gpu.device, &gpu.queue, &mut atlas, &content, (16, 16));
    let output = Offscreen::with_format(&gpu.device, 16, 16, wgpu::TextureFormat::Bgra8Unorm);
    assert!(pixels(&gpu, &renderer, &output).iter().all(|v| *v == 255));
}

#[test]
fn glass_only_warmup_keeps_float_backdrops_lazy_and_reuses_the_content_pipeline() {
    let gpu = Gpu::headless();
    let mut renderer = Renderer::new(&gpu.device, OFFSCREEN_FORMAT);
    let mut rt = crate::Runtime::new(
        (),
        |_, _| crate::el().size(48.0, 48.0).backdrop_blur(3.0),
        (64.0, 64.0),
        crate::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    assert!(
        rt.instances
            .iter()
            .any(|i| i.params[1] == KIND_BACKDROP_BLUR)
    );
    renderer.prepare(
        &gpu.device,
        &gpu.queue,
        &mut rt.text.atlas,
        &rt.instances,
        (64, 64),
    );
    finish(&mut renderer, &gpu.device);
    let warmed = match &renderer.effect_warmup {
        State::Done {
            content: Some(pipeline),
        } => pipeline.clone(),
        _ => panic!("float content pipeline was not retained"),
    };
    renderer.prepare(
        &gpu.device,
        &gpu.queue,
        &mut rt.text.atlas,
        &rt.instances,
        (64, 64),
    );
    assert_eq!(renderer.layer_stats().bytes, 0);
    assert_eq!(renderer.backdrops.len(), 1);
    assert_eq!(renderer.backdrops[0].format, OFFSCREEN_FORMAT);
    assert!(!renderer.extra.iter().any(|(f, _)| *f == layers::FORMAT));

    let mut layered = vec![
        Instance::begin_layer(
            [4.0, 4.0, 32.0, 32.0],
            NO_CLIP,
            LayerSpec {
                opacity: 0.5,
                ..Default::default()
            },
        ),
        rect([4.0, 4.0, 32.0, 32.0], [1.0; 4]),
        Instance::end_layer(),
    ];
    layered.extend_from_slice(&rt.instances);
    renderer.prepare(
        &gpu.device,
        &gpu.queue,
        &mut rt.text.atlas,
        &layered,
        (64, 64),
    );
    assert!(renderer.layer_stats().bytes > 0);
    assert!(
        renderer
            .backdrops
            .iter()
            .any(|b| b.format == layers::FORMAT)
    );
    assert_eq!(
        renderer
            .extra
            .iter()
            .find(|(f, _)| *f == layers::FORMAT)
            .unwrap()
            .1,
        warmed,
        "the first isolated layer takes the compiled content pipeline"
    );
    assert!(matches!(
        renderer.effect_warmup,
        State::Done { content: None }
    ));
    let output = Offscreen::new(&gpu.device, 64, 64);
    assert!(pixels(&gpu, &renderer, &output).iter().any(|v| *v != 0));
}
