use crate::error::AppResult;
use crate::models::account::{
    Account, BRANDING_KEY, EMAIL_BRANDING_KEY, SUPPORT_LINKS_KEY,
};
use crate::state::AppState;
use regex::Regex;
use serde_json::{json, Value};

pub const DEFAULT_PRODUCT_NAME: &str = "Signline";
pub const DEFAULT_PRIMARY_COLOR: &str = "#2161AD";
pub const DEFAULT_ACCENT_COLOR: &str = "#253E8A";
pub const UPSTREAM_NAME: &str = "Signline";
pub const UPSTREAM_URL: &str = "https://github.com/signline/signline";

/// Validates a `#RGB` or `#RRGGBB` hex color string.
pub fn is_valid_hex_color(color: &str) -> bool {
    let re = Regex::new(r"^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$").unwrap();
    re.is_match(color.trim())
}

/// Converts a `#RGB` or `#RRGGBB` string into `(r, g, b)` bytes.
pub fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let clean = hex.trim().trim_start_matches('#');
    let expanded = if clean.len() == 3 {
        let chars: Vec<char> = clean.chars().collect();
        format!(
            "{}{}{}{}{}{}",
            chars[0], chars[0], chars[1], chars[1], chars[2], chars[2]
        )
    } else if clean.len() == 6 {
        clean.to_string()
    } else {
        "2161AD".to_string()
    };

    let r = u8::from_str_radix(&expanded[0..2], 16).unwrap_or(0x21);
    let g = u8::from_str_radix(&expanded[2..4], 16).unwrap_or(0x61);
    let b = u8::from_str_radix(&expanded[4..6], 16).unwrap_or(0xAD);
    (r, g, b)
}

/// Computes relative luminance and chooses dark (`#1f2937`) or light (`#ffffff`) foreground text
/// using Signline's contrast threshold (`0.62`).
pub fn content_color(hex: &str) -> &'static str {
    let (r, g, b) = hex_to_rgb(hex);
    let luminance = ((0.299 * (r as f64)) + (0.587 * (g as f64)) + (0.114 * (b as f64))) / 255.0;
    if luminance >= 0.62 {
        "#1f2937"
    } else {
        "#ffffff"
    }
}

/// Converts a hex color into space-separated HSL values (`210.00 68.00% 40.39%`) suitable for
/// both ShadCN/UI (`--primary`) and DaisyUI (`--p`) CSS custom properties.
pub fn hex_to_hsl(hex: &str) -> String {
    let (r_u8, g_u8, b_u8) = hex_to_rgb(hex);
    let r = (r_u8 as f64) / 255.0;
    let g = (g_u8 as f64) / 255.0;
    let b = (b_u8 as f64) / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let lightness = (max + min) / 2.0;

    let saturation = if delta.abs() < f64::EPSILON {
        0.0
    } else {
        delta / (1.0 - ((2.0 * lightness) - 1.0).abs())
    };

    let mut hue = if delta.abs() < f64::EPSILON {
        0.0
    } else if (max - r).abs() < f64::EPSILON {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < f64::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    if hue < 0.0 {
        hue += 360.0;
    }

    format!(
        "{:.2} {:.2}% {:.2}%",
        hue,
        saturation * 100.0,
        lightness * 100.0
    )
}

/// Resolves the complete white-label branding payload for an account (or the instance default),
/// always including the mandatory AGPLv3 Section 7(b) upstream Signline attribution.
pub async fn resolve_branding(state: &AppState, account: Option<&Account>) -> AppResult<Value> {
    let resolved_acc = match account {
        Some(a) => Some(a.clone()),
        None => state.pb.resolve_account(None).await?,
    };

    let acc_id = resolved_acc.as_ref().map(|a| a.id.as_str());

    let branding_cfg = state
        .pb
        .get_account_config(acc_id, BRANDING_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    let email_cfg = state
        .pb
        .get_account_config(acc_id, EMAIL_BRANDING_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    let support_cfg = state
        .pb
        .get_account_config(acc_id, SUPPORT_LINKS_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    let product_name = branding_cfg
        .get("product_name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_PRODUCT_NAME)
        .to_string();

    let primary_color = branding_cfg
        .get("primary_color")
        .and_then(|v| v.as_str())
        .filter(|c| is_valid_hex_color(c))
        .unwrap_or(DEFAULT_PRIMARY_COLOR)
        .to_string();

    let accent_color = branding_cfg
        .get("accent_color")
        .and_then(|v| v.as_str())
        .filter(|c| is_valid_hex_color(c))
        .unwrap_or(DEFAULT_ACCENT_COLOR)
        .to_string();

    let primary_fg = content_color(&primary_color);
    let accent_fg = content_color(&accent_color);

    let has_custom_logo = resolved_acc
        .as_ref()
        .map(|a| !a.logo_file.is_empty())
        .unwrap_or(false);

    let has_custom_favicon = resolved_acc
        .as_ref()
        .map(|a| !a.favicon_file.is_empty())
        .unwrap_or(false);

    let css_variables = json!({
        "--primary": hex_to_hsl(&primary_color),
        "--primary-foreground": hex_to_hsl(primary_fg),
        "--accent": hex_to_hsl(&accent_color),
        "--accent-foreground": hex_to_hsl(accent_fg),
        "--p": hex_to_hsl(&primary_color),
        "--pf": hex_to_hsl(&primary_color),
        "--pc": hex_to_hsl(primary_fg),
        "--a": hex_to_hsl(&accent_color),
        "--af": hex_to_hsl(&accent_color),
        "--ac": hex_to_hsl(accent_fg),
    });

    let sender_name = email_cfg.get("sender_name").and_then(|v| v.as_str()).unwrap_or("");
    let footer = email_cfg.get("footer").and_then(|v| v.as_str()).unwrap_or("");
    let support_email = support_cfg.get("support_email").and_then(|v| v.as_str()).unwrap_or("");
    let support_url = support_cfg.get("support_url").and_then(|v| v.as_str()).unwrap_or("");
    let terms_url = support_cfg.get("terms_url").and_then(|v| v.as_str()).unwrap_or("");
    let privacy_url = support_cfg.get("privacy_url").and_then(|v| v.as_str()).unwrap_or("");

    Ok(json!({
        "product_name": product_name,
        "primary_color": primary_color,
        "accent_color": accent_color,
        "primary_foreground": primary_fg,
        "accent_foreground": accent_fg,
        "custom_colors": branding_cfg.get("primary_color").is_some() || branding_cfg.get("accent_color").is_some(),
        "has_custom_logo": has_custom_logo,
        "has_custom_favicon": has_custom_favicon,
        "logo_url": "/branding/logo",
        "favicon_url": "/branding/favicon",
        "css_variables": css_variables,
        "email_sender_name": sender_name,
        "email_footer": footer,
        "support_email": support_email,
        "support_url": support_url,
        "terms_url": terms_url,
        "privacy_url": privacy_url,
        "email_branding": {
            "sender_name": sender_name,
            "footer": footer,
        },
        "support_links": {
            "support_email": support_email,
            "support_url": support_url,
            "terms_url": terms_url,
            "privacy_url": privacy_url,
        },
        "attribution": {
            "product_name": product_name,
            "upstream_name": UPSTREAM_NAME,
            "upstream_url": UPSTREAM_URL,
            "text": format!("Powered by {}, based on {}", product_name, UPSTREAM_NAME),
            "license": "AGPL-3.0-only"
        }
    }))
}
