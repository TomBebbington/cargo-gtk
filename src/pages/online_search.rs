use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};


use crates_io::{Crate, Registry};

use cargo::core::source::SourceId;
use cargo::ops::{self, CompileOptions};

use cargo::util::config::Config;

use gtk::prelude::*;
use gtk::{Window, SearchEntry, ListStore, TreeView, Button};

use pages::Page;

use thread_scoped::scoped;

use util::{Options, PreContext};

fn get_registry() -> Registry {
    Registry::new("https://crates.io".into(), None)
}


/// An interface for searching online for packages.
#[derive(Clone)]
pub struct OnlineSearchPage {
    pub config: Rc<Config>,
    pub options: Rc<RefCell<Options>>,
    pub window: Window,
    pub search: SearchEntry,
    pub store: ListStore,
    pub packages: TreeView,
    pub install: Button,
    pub sender: Sender<Vec<Crate>>,
    pub receiver: Rc<Receiver<Vec<Crate>>>,
    results: Rc<RefCell<Vec<Crate>>>
}
impl Page for OnlineSearchPage {
    fn new(c: &PreContext) -> Self {
        let (tx, rx) = channel();
        OnlineSearchPage {
            config: c.config.clone(),
            options: c.options.clone(),
            window: c.window.clone(),
            search: c.builder.get_object("online_search").unwrap(),
            packages: c.builder.get_object("online_packages").unwrap(),
            install: c.builder.get_object("install").unwrap(),
            store: c.builder.get_object("store").unwrap(),
            sender: tx,
            receiver: Rc::new(rx),
            results: Rc::new(RefCell::new(Vec::new()))
        }
    }
    fn update(&self) {
        if let Ok(crates) = self.receiver.try_recv() {
            for old_result in self.packages.get_children() {
                self.packages.remove(&old_result);
            }
            let mut results = self.results.borrow_mut();
            *results = crates;
            self.store.clear();
            for c in &*results {
                self.store.insert_with_values(None, &[0, 1, 2], &[&c.name, &c.description, &c.max_version]);
            }
            self.packages.show_all();
        }
    }
    fn bind_listeners(&self) {
        let results = self.results.clone();
        let window = self.window.clone();
        let config = self.config.clone();
        let options = self.options.clone();
        let packages = self.packages.clone();
        self.install.connect_clicked(move |_| {
            let index = packages.get_cursor().0.unwrap().get_indices()[0];
            let results = results.borrow();
            let id = SourceId::for_central(&*config).unwrap();
            let options = options.borrow();
            let options: CompileOptions = (&*options).into();
            let name = &results[index as usize].name;
            if let Err(err) = ops::install(None, Some(name), &id, None, &options) {
                error!(Some(&window), "Failed to install '{}': {:?}", name, err);
            } else {
                info!(Some(&window), "Crate '{}' successfully installed", name);
            }
        });
        let thread = RefCell::new(None);
        let sender = self.sender.clone();
        self.search.connect_activate(move |entry| {
            let query = entry.get_text().unwrap_or_else(|| "".to_string());
            let tx = Arc::new(Mutex::new(sender.clone()));
            let mut thread = thread.borrow_mut();
            *thread = Some(unsafe { scoped(move || {
                let mut registry = get_registry();
                let tx = tx.lock().unwrap();
                tx.send(registry.search(&query, 64).map_err(|_| "Search failed").unwrap().0).unwrap();
            })});
        });
    }
}
