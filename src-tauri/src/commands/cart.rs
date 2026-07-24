//! Cart automation — deliberately built on deep links opened in the user's
//! logged-in Chrome, NOT scraping/automation (which retailers block and which
//! breaks constantly). Amazon's cart-add URL drops an item straight into the
//! signed-in cart; other retailers open a product search so the user clicks
//! "Add to Cart" themselves.

use crate::commands::browser::{open_urls_impl, urlencode};

/// Amazon item numbers (ASINs) are 10-char alphanumerics with at least one
/// digit — when the "item" is one of these we can add it to the cart directly.
fn is_asin(s: &str) -> bool {
    let s = s.trim();
    s.len() == 10
        && s.chars().all(|c| c.is_ascii_alphanumeric())
        && s.chars().any(|c| c.is_ascii_digit())
        && s.chars().any(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

fn retailer_url(retailer: &str, item: &str) -> Option<String> {
    let q = urlencode(item);
    Some(match retailer.trim().to_lowercase().as_str() {
        "amazon" => {
            if is_asin(item) {
                // Amazon's remote "Add to Cart" form — lands the item in the
                // signed-in cart and shows it.
                format!(
                    "https://www.amazon.com/gp/aws/cart/add.html?ASIN.1={}&Quantity.1=1",
                    item.trim()
                )
            } else {
                format!("https://www.amazon.com/s?k={q}")
            }
        }
        "walmart" => format!("https://www.walmart.com/search?q={q}"),
        "newegg" => format!("https://www.newegg.com/p/pl?d={q}"),
        "microcenter" | "micro center" => {
            format!("https://www.microcenter.com/search/search_results.aspx?Ntt={q}")
        }
        "apple" | "apple store" => format!("https://www.apple.com/us/search/{q}"),
        "bestbuy" | "best buy" => format!("https://www.bestbuy.com/site/searchpage.jsp?st={q}"),
        "target" => format!("https://www.target.com/s?searchTerm={q}"),
        _ => return None,
    })
}

pub fn add_to_cart_impl(retailer: String, item: String) -> Result<String, String> {
    let item = item.trim().to_string();
    if item.is_empty() {
        return Err("Nothing to add to the cart.".to_string());
    }
    let url = retailer_url(&retailer, &item)
        .ok_or_else(|| format!("I don't have a cart shortcut for \"{retailer}\" yet."))?;
    open_urls_impl(vec![url])?;

    if retailer.trim().eq_ignore_ascii_case("amazon") && is_asin(&item) {
        Ok(format!("Adding item {item} to your Amazon cart."))
    } else {
        Ok(format!(
            "Opened {} for \"{item}\" — click Add to Cart to finish.",
            retailer.trim()
        ))
    }
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn add_to_cart(retailer: String, item: String) -> Result<String, String> {
    add_to_cart_impl(retailer, item)
}
