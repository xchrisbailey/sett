//! FORK: colour maths for text drawn on a tint: blending, WCAG contrast,
//! and shading a colour until it reads.

use gpui::Hsla;

/// WCAG AA for normal-size text.
pub(crate) const MIN_CONTRAST: f32 = 4.5;

/// `top`, with its alpha, laid over the opaque `bottom`.
pub(crate) fn over(top: Hsla, bottom: Hsla) -> Hsla {
    let (t, b) = (top.to_rgb(), bottom.to_rgb());
    let mix = |t: f32, b: f32| t * top.a + b * (1. - top.a);
    gpui::Rgba {
        r: mix(t.r, b.r),
        g: mix(t.g, b.g),
        b: mix(t.b, b.b),
        a: 1.,
    }
    .into()
}

/// `color`, darkened (on a light background) or lightened (on a dark one)
/// until it reaches [`MIN_CONTRAST`] on `background`.
pub(crate) fn readable(color: Hsla, background: Hsla) -> Hsla {
    let darken = luminance(background) > 0.5;
    let mut text = color;
    text.a = 1.;
    for _ in 0..50 {
        if contrast(text, background) >= MIN_CONTRAST {
            break;
        }
        text.l = if darken {
            (text.l - 0.02).max(0.)
        } else {
            (text.l + 0.02).min(1.)
        };
    }
    text
}

/// WCAG relative luminance.
fn luminance(color: Hsla) -> f32 {
    let rgb = color.to_rgb();
    let channel = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b)
}

/// WCAG contrast ratio.
fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::rgb;

    /// Default light: white page; default dark: neutral-950.
    const LIGHT: u32 = 0xffffff;
    const DARK: u32 = 0x0a0a0a;
    /// The default themes' info, success, magenta, warning and danger.
    const LIGHT_COLORS: [u32; 5] = [0x06b6d4, 0x22c55e, 0x9333ea, 0xeab308, 0xef4444];
    const DARK_COLORS: [u32; 5] = [0x22d3ee, 0x4ade80, 0xc084fc, 0xfacc15, 0xf87171];

    #[test]
    fn callout_header_text_reads_on_its_tint_in_the_default_themes() {
        for (page, colors) in [(LIGHT, LIGHT_COLORS), (DARK, DARK_COLORS)] {
            for c in colors {
                let base: Hsla = rgb(c).into();
                let box_bg = over(base.opacity(crate::callout::TINT), rgb(page).into());
                let label = readable(base, box_bg);
                assert!(
                    contrast(label, box_bg) >= MIN_CONTRAST,
                    "{c:06x} on {page:06x}: {}",
                    contrast(label, box_bg)
                );
            }
        }
    }
}
