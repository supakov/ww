//! The renderers built into the executable and the start of a window with them. OpenGL (glow)
//! takes less memory but needs a driver of OpenGL 2.0 or later; DirectX 12 (wgpu with its DirectX
//! 12 backend only) also draws without a driver of the graphics card, on the software rasterizer of
//! Windows. Automatic tries them in that order and takes the first that starts; a chosen one is
//! tried first, and the others follow when it does not start.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;

#[cfg(not(any(feature = "glow", feature = "wgpu")))]
compile_error!("winweaver-ui needs a renderer: the feature glow, wgpu or both");

/// A renderer of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renderer {
    #[cfg(feature = "glow")]
    OpenGl,
    #[cfg(feature = "wgpu")]
    DirectX,
}

impl Renderer {
    /// The renderers of this executable, in the order Automatic tries them.
    pub const ALL: &[Renderer] = &[
        #[cfg(feature = "glow")]
        Renderer::OpenGl,
        #[cfg(feature = "wgpu")]
        Renderer::DirectX,
    ];

    /// The name of the settings and of `--renderer`.
    pub fn id(self) -> &'static str {
        match self {
            #[cfg(feature = "glow")]
            Renderer::OpenGl => "opengl",
            #[cfg(feature = "wgpu")]
            Renderer::DirectX => "directx",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            #[cfg(feature = "glow")]
            Renderer::OpenGl => "OpenGL",
            #[cfg(feature = "wgpu")]
            Renderer::DirectX => "DirectX 12",
        }
    }

    /// The renderer of a name, if this executable has it.
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        Self::ALL.iter().copied().find(|r| r.id() == name)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn eframe(self) -> eframe::Renderer {
        match self {
            #[cfg(feature = "glow")]
            Renderer::OpenGl => eframe::Renderer::Glow,
            #[cfg(feature = "wgpu")]
            Renderer::DirectX => eframe::Renderer::Wgpu,
        }
    }
}

/// The renderers to try for a choice: the chosen one first, then the other ones of the
/// executable; every one of them for Automatic (`None`).
pub fn order(choice: Option<Renderer>) -> Vec<Renderer> {
    let mut order: Vec<Renderer> = choice.into_iter().collect();
    order.extend(Renderer::ALL.iter().copied().filter(|r| Some(*r) != choice));
    order
}

/// What the window draws with, for the log and the settings: the version of OpenGL and its
/// renderer, or the adapter wgpu chose (a software one is `Cpu`) with its driver and the other
/// adapters it found.
pub fn graphics_text(cc: &eframe::CreationContext<'_>) -> String {
    #[cfg(feature = "wgpu")]
    if let Some(state) = &cc.wgpu_render_state {
        let a = state.adapter.get_info();
        let all: Vec<String> = state
            .available_adapters
            .iter()
            .map(|o| {
                let o = o.get_info();
                format!("{} ({:?}, {:?})", o.name, o.backend, o.device_type)
            })
            .collect();
        return format!(
            "DirectX 12: {} ({:?}, {:?}), driver {} {}; available: {}",
            a.name,
            a.backend,
            a.device_type,
            a.driver,
            a.driver_info,
            all.join(", ")
        );
    }
    #[cfg(feature = "glow")]
    if let Some(gl) = &cc.gl {
        use eframe::glow::HasContext;
        // SAFETY: the context of the window is current while the app is created.
        let (version, renderer) = unsafe {
            (
                gl.get_parameter_string(eframe::glow::VERSION),
                gl.get_parameter_string(eframe::glow::RENDERER),
            )
        };
        return format!("OpenGL {version}, {renderer}");
    }
    let _ = cc;
    "no renderer".to_owned()
}

/// Opens a window with the first renderer of `order` that starts. `make` gives the creator of the
/// app for a renderer and the failures of the ones tried before it; it is called once per try. A
/// renderer fails when the window ends with an error before its app is created; an error after
/// that ends the start. The renderer that drew and the failures before it. A browser starts
/// through `web::start` with WebGL.
#[cfg(not(target_arch = "wasm32"))]
pub fn start(
    title: &str,
    viewport: &egui::ViewportBuilder,
    order: &[Renderer],
    mut make: impl FnMut(Renderer, &[String]) -> eframe::AppCreator<'static>,
) -> Result<(Renderer, Vec<String>), String> {
    let mut failures = Vec::new();
    for &renderer in order {
        let created = Arc::new(AtomicBool::new(false));
        let flag = created.clone();
        let inner = make(renderer, &failures);
        let creator: eframe::AppCreator<'static> = Box::new(move |cc| {
            flag.store(true, Ordering::SeqCst);
            inner(cc)
        });
        let options = eframe::NativeOptions {
            viewport: viewport.clone(),
            renderer: renderer.eframe(),
            ..Default::default()
        };
        match eframe::run_native(title, options, creator) {
            Ok(()) => return Ok((renderer, failures)),
            Err(e) if !created.load(Ordering::SeqCst) => {
                let text = format!("{} did not start: {e}", renderer.title());
                failures.push(text);
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Err(failures.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chosen_renderer_comes_first_and_the_others_follow() {
        assert_eq!(order(None), Renderer::ALL.to_vec());
        for &r in Renderer::ALL {
            let o = order(Some(r));
            assert_eq!(o[0], r);
            assert_eq!(o.len(), Renderer::ALL.len());
            assert_eq!(Renderer::parse(&r.id().to_uppercase()), Some(r));
        }
        assert_eq!(Renderer::parse("vulkan"), None);
    }
}
