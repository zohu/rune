//! Compile effect pipelines after preparing the first frame, without moving UI state to the worker.
use super::*;

#[derive(Default)]
pub(super) enum State {
    #[default]
    FirstFrame,
    Idle,
    #[cfg(not(target_arch = "wasm32"))]
    Running(std::sync::mpsc::Receiver<Ready>),
    Done {
        content: Option<wgpu::RenderPipeline>,
    },
}

impl State {
    pub(super) fn take_content(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> Option<wgpu::RenderPipeline> {
        match self {
            Self::Done { content } if format == layers::FORMAT => content.take(),
            _ => None,
        }
    }
}

pub(super) struct Ready {
    layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    content: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    additive: (wgpu::RenderPipeline, wgpu::RenderPipeline),
    blur: layer_blur::Pipeline,
    backdrop: Option<backdrop_blur::BackdropBlur>,
}

impl Renderer {
    /// Never wait for a worker. An effect used before it finishes keeps its lazy fallback.
    pub(super) fn prepare_effect_pipelines(&mut self, device: &wgpu::Device) {
        match &self.effect_warmup {
            State::FirstFrame => {
                self.effect_warmup = State::Idle;
                return;
            }
            State::Done { .. } => return,
            #[cfg(not(target_arch = "wasm32"))]
            State::Running(rx) => {
                use std::sync::mpsc::TryRecvError;
                let ready = match rx.try_recv() {
                    Ok(ready) => ready,
                    Err(TryRecvError::Empty) => return,
                    Err(TryRecvError::Disconnected) => {
                        self.effect_warmup = State::Done { content: None };
                        return;
                    }
                };
                self.effect_warmup = State::Done { content: None };
                if self.layout != ready.layout || self.format != ready.format {
                    return;
                }
                if self.format != layers::FORMAT
                    && !self.extra.iter().any(|(f, _)| *f == layers::FORMAT)
                {
                    // A compiled format is not an active render target: registering it in
                    // extra would also allocate its full-sized backdrop on glass-only frames.
                    self.effect_warmup = State::Done {
                        content: Some(ready.content),
                    };
                }
                if !self
                    .layers
                    .pipelines
                    .iter()
                    .any(|(f, _)| *f == layers::FORMAT)
                {
                    self.layers
                        .pipelines
                        .push((layers::FORMAT, ready.composite));
                }
                self.add_layer_format(device, self.format);
                self.layers.additive.get_or_insert(ready.additive);
                self.layers.blur.get_or_insert(ready.blur);
                if self.backdrop_blur.is_none() {
                    if let Some(mut backdrop) = ready.backdrop {
                        for bd in &self.backdrops {
                            backdrop.bind_backdrop(device, bd.format, &bd.view);
                        }
                        self.backdrop_blur = Some(backdrop);
                    }
                }
                return;
            }
            State::Idle => {}
        }
        self.effect_warmup = State::Done { content: None };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let device = device.clone();
            let layout = self.layout.clone();
            let bind_layout = self.bind_layout.clone();
            let shader = self.shader.clone();
            let format = self.format;
            let vertex = self.vertex_entry;
            let needs_backdrop = self.backdrop_blur.is_none();
            let (tx, rx) = std::sync::mpsc::channel();
            if std::thread::Builder::new()
                .name("rune-effect-pipelines".into())
                .spawn(move || {
                    let build = || {
                        let pipeline = |entry, blend| {
                            Self::pipeline(&device, &layout, &shader, layers::FORMAT, entry, blend)
                        };
                        let content = pipeline(vertex, Blend::SourceOver);
                        let composite = if vertex == "vs" {
                            content.clone()
                        } else {
                            pipeline("vs", Blend::SourceOver)
                        };
                        let add = pipeline(vertex, Blend::PlusLighter);
                        let additive = if vertex == "vs" {
                            (add.clone(), add)
                        } else {
                            (add, pipeline("vs", Blend::PlusLighter))
                        };
                        let blur = layer_blur::Pipeline::new(&device);
                        let backdrop = needs_backdrop.then(|| {
                            let mut blur = backdrop_blur::BackdropBlur::new(&device, &bind_layout);
                            blur.ensure_composite(&device, format);
                            blur.ensure_composite(&device, layers::FORMAT);
                            blur
                        });
                        let _ = tx.send(Ready {
                            layout,
                            format,
                            content,
                            composite,
                            additive,
                            blur,
                            backdrop,
                        });
                    };
                    #[cfg(target_os = "macos")]
                    objc2::rc::autoreleasepool(|_| build());
                    #[cfg(not(target_os = "macos"))]
                    build();
                })
                .is_ok()
            {
                self.effect_warmup = State::Running(rx);
            }
        }
    }
}

#[cfg(test)]
#[path = "effect_warmup_tests.rs"]
mod tests;
