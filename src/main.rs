/*
Copyright 2016 Tom Bebbington

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

  http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/
extern crate gtk;
extern crate thread_scoped;
extern crate cargo;
extern crate crates_io;

macro_rules! error {
    ($window:expr, $($tt:tt)*) => (::error($window, &format!($($tt)*)))
}
macro_rules! info {
    ($window:expr, $($tt:tt)*) => (::info($window, &format!($($tt)*)))
}
mod util;
mod pages;

use std::path::Path;
use std::cell::RefCell;
use std::str;
use std::rc::Rc;
use std::default::Default;

use util::{Options, PreContext};

use pages::{Page, OnlineSearchPage, LocalPackagePage, NewPackagePage};

use cargo::util::config::Config;

use gtk::prelude::*;
use gtk::{DIALOG_DESTROY_WITH_PARENT, DIALOG_MODAL, Notebook, ButtonsType, Builder, MessageType, MessageDialog, CellRendererText, FileChooser, TreeView, Button, ListStore, TreeViewColumn, Window};

fn make_column(title: &str, kind: &str, id: i32) -> TreeViewColumn {
    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();

    column.set_title(title);
    column.pack_start(&cell, true);
    column.add_attribute(&cell, kind, id);
    column.set_sort_column_id(id);
    column
}

fn dialog(parent: Option<&Window>, ty: MessageType, text: &str) {
    let dialog = MessageDialog::new(parent, DIALOG_DESTROY_WITH_PARENT | DIALOG_MODAL, ty, ButtonsType::Close, text);
    dialog.connect_response(|dialog, _| {
        dialog.destroy();
    });
    dialog.set_icon_name(Some("dialog_error"));
    dialog.set_deletable(true);
    dialog.show_all();
    dialog.present();
    dialog.run();
}

fn error(parent: Option<&Window>, text: &str) {
    dialog(parent, MessageType::Error, text)
}
fn info(parent: Option<&Window>, text: &str) {
    dialog(parent, MessageType::Info, text)
}

fn bind_file_button<F>(file: &FileChooser, button: &Button, f: F) where F: Fn(&Path) + 'static {
    let file = file.clone();
    button.connect_clicked(move |_| {
        if let Some(file) = file.get_filename() {
            f(&file);
        }
    });
}

#[derive(Clone)]
pub struct Context {
    pub config: Rc<Config>,
    pub builder: Builder,
    pub window: Window,
    pub tabs: Notebook,
    pub online_packs: OnlineSearchPage,
    pub local_pack: LocalPackagePage,
    pub new_pack: NewPackagePage,
    pub options: Rc<RefCell<Options>>
}

impl Context {
    fn new() -> Context {
        let builder = Builder::new_from_string(include_str!("layout.glade"));
        let packs: TreeView = builder.get_object("online_packages").expect("Failed to load online_packages");
        let store: ListStore = builder.get_object("store").unwrap();
        packs.set_headers_visible(true);
        packs.append_column(&make_column("Package", "text", 0));
        packs.append_column(&make_column("Description", "text", 1));
        packs.append_column(&make_column("Version", "text", 2));
        packs.set_model(Some(&store));
        let tabs = builder.get_object("tabs").unwrap();
        let pre = PreContext {
            window: builder.get_object("window").unwrap(),
            builder: builder,
            config: Rc::new(Config::default().unwrap()),
            options: Rc::new(RefCell::new(Options::default()))
        };
        let local_pack = LocalPackagePage::new(&pre);
        let online_packs = OnlineSearchPage::new(&pre);
        let new_pack = NewPackagePage::new(&pre);
        Context {
            window: pre.window,
            builder: pre.builder,
            config: pre.config,
            options: pre.options,
            tabs: tabs,
            local_pack: local_pack,
            online_packs: online_packs,
            new_pack: new_pack
        }
    }
    fn bind_listeners(&self) {
        self.online_packs.bind_listeners();
        self.local_pack.bind_listeners();
        self.new_pack.bind_listeners();
        let online_packs = self.online_packs.clone();
        let local_pack = self.local_pack.clone();
        let new_pack = self.new_pack.clone();
        self.window.connect_draw(move |_, _| {
            online_packs.update();
            local_pack.update();
            new_pack.update();
            Inhibit(false)
        });
        self.window.connect_delete_event(|_, _| {
            gtk::main_quit();
            Inhibit(false)
        });
    }
}

fn main() {
    gtk::init().unwrap_or_else(|_| panic!("{}", "cargo-manager: failed to initialize GTK."));
    let c = Context::new();
    c.bind_listeners();
    c.window.show_all();
    gtk::main();
}