// SPDX-License-Identifier: GPL-3.0-or-later

//! What the app asks of the graphics card: [`wgpu_setup`], which eframe
//! opens the card with, to draw the window.
//!
//! A photo shown at 100 % from its largest preview needs a texture as
//! large as that preview, and some cameras make previews more than 8192
//! pixels wide. egui asks every card for textures of 8192 pixels a side,
//! whatever the card supports, so the app asks instead for the largest
//! the card itself reports ([`device_limits`]): at least 16384 on every
//! card of the Vulkan, DirectX 12 or Metal class. Everything else is
//! asked as egui asks it.

use std::sync::Arc;

use eframe::egui_wgpu::{WgpuSetup, WgpuSetupCreateNew, wgpu};

/// egui's own way of opening the graphics card, asking for the largest
/// textures the card supports.
pub fn wgpu_setup() -> WgpuSetup {
    WgpuSetupCreateNew {
        device_descriptor: Arc::new(|adapter| wgpu::DeviceDescriptor {
            label: Some("egui wgpu device"),
            required_limits: device_limits(adapter.get_info().backend, &adapter.limits()),
            ..Default::default()
        }),
        ..WgpuSetupCreateNew::without_display_handle()
    }
    .into()
}

/// The limits to ask a card for, given its backend and the limits it
/// reports: egui's, but with textures as large as the card supports. A
/// card is never asked for more than it reports, which would fail.
///
/// On OpenGL, the fallback for cards without Vulkan, DirectX 12 or Metal,
/// egui's limits are kept as they are.
fn device_limits(backend: wgpu::Backend, adapter: &wgpu::Limits) -> wgpu::Limits {
    if backend == wgpu::Backend::Gl {
        wgpu::Limits {
            max_texture_dimension_2d: 8192,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        }
    } else {
        wgpu::Limits {
            max_texture_dimension_2d: adapter.max_texture_dimension_2d,
            ..wgpu::Limits::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The limits a card reports, with textures of `side` pixels a side
    /// and, as a real card does, more than egui asks for elsewhere too.
    fn card(side: u32) -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_1d: side,
            max_texture_dimension_2d: side,
            max_bind_groups: 8,
            max_buffer_size: 1 << 40,
            ..wgpu::Limits::default()
        }
    }

    #[test]
    fn asks_for_textures_as_large_as_the_card_supports() {
        for backend in [
            wgpu::Backend::Vulkan,
            wgpu::Backend::Dx12,
            wgpu::Backend::Metal,
        ] {
            for side in [16384, 32768] {
                let limits = device_limits(backend, &card(side));
                assert_eq!(limits.max_texture_dimension_2d, side, "{backend:?}");
            }
        }
    }

    #[test]
    fn never_asks_for_larger_textures_than_the_card_supports() {
        let limits = device_limits(wgpu::Backend::Vulkan, &card(4096));
        assert_eq!(limits.max_texture_dimension_2d, 4096);
    }

    #[test]
    fn asks_for_everything_else_as_egui_does() {
        let limits = device_limits(wgpu::Backend::Metal, &card(16384));
        assert_eq!(
            wgpu::Limits {
                max_texture_dimension_2d: 8192,
                ..limits
            },
            wgpu::Limits {
                max_texture_dimension_2d: 8192,
                ..wgpu::Limits::default()
            },
        );
    }

    #[test]
    fn keeps_eguis_limits_on_opengl() {
        let limits = device_limits(wgpu::Backend::Gl, &card(16384));
        assert_eq!(
            limits,
            wgpu::Limits {
                max_texture_dimension_2d: 8192,
                ..wgpu::Limits::downlevel_webgl2_defaults()
            },
        );
    }
}
