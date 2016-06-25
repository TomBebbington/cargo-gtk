mod create;
mod local;
mod online_search;

use util::PreContext;
pub use pages::create::NewPackagePage;
pub use pages::local::LocalPackagePage;
pub use pages::online_search::OnlineSearchPage;

pub trait Page: Clone {
	/// Create a new page with the window, builder, etc from the `PreContext` given.
    fn new(context: &PreContext) -> Self;
    /// Update this page's display.
    fn update(&self) {}
    /// Check if this page should be updated.
    fn should_update() -> bool { false }
    /// Bind listeners for this page.
    fn bind_listeners(&self) {}
}