use std::cell::Cell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4_layer_shell::{Layer, LayerShell};
use webkit6::prelude::*;

const HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><style>
html,body{margin:0;background:#000;overflow:hidden}
#host,iframe{width:100vw;height:100vh;border:0}
</style></head><body><div id="host"></div><script>
let current=null, frame=null, lastPos=0;
function apply(id,pos,playing){
  const position=Math.max(0, Math.floor(pos/1000));
  if(current!==id){
    current=id; lastPos=position;
    document.getElementById('host').innerHTML='';
    frame=document.createElement('iframe');
    frame.allow='autoplay; encrypted-media; picture-in-picture';
    frame.allowFullscreen=true;
    frame.src='https://www.youtube-nocookie.com/embed/'+encodeURIComponent(id)
      +'?enablejsapi=1&autoplay='+(playing?1:0)+'&start='+position+'&rel=0';
    document.getElementById('host').appendChild(frame);
    return;
  }
  if(!frame||!frame.contentWindow) return;
  const command=playing?'playVideo':'pauseVideo';
  frame.contentWindow.postMessage(JSON.stringify({event:'command',func:command,args:[]}),'*');
  if(Math.abs(position-lastPos)>2){
    lastPos=position;
    frame.contentWindow.postMessage(JSON.stringify({event:'command',func:'seekTo',args:[position,true]}),'*');
  }
}
window.ticonnectApply=apply;
</script></body></html>"#;

pub struct Player {
    window: gtk::Window,
    webview: webkit6::WebView,
    ready: Rc<Cell<bool>>,
    pending: Rc<std::cell::RefCell<Option<(String, u64, bool)>>>,
}

impl Player {
    pub fn new(application: &gtk::Application) -> Self {
        let window = gtk::Window::builder()
            .application(application)
            .decorated(false)
            .resizable(true)
            .default_width(640)
            .default_height(400)
            .build();
        window.add_css_class("ticonnect-player");
        window.init_layer_shell();
        window.set_anchor(gtk4_layer_shell::Edge::Top, true);
        window.set_anchor(gtk4_layer_shell::Edge::Right, true);
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);
        window.set_exclusive_zone(0);
        window.set_margin(gtk4_layer_shell::Edge::Top, 24);
        window.set_margin(gtk4_layer_shell::Edge::Right, 24);
        window.set_namespace(Some("ticonnect-player"));
        window.set_visible(false);

        let session = webkit6::NetworkSession::new_ephemeral();
        let settings = webkit6::Settings::new();
        settings.set_allow_file_access_from_file_urls(false);
        settings.set_allow_universal_access_from_file_urls(false);
        settings.set_enable_developer_extras(false);
        let webview = webkit6::WebView::builder()
            .network_session(&session)
            .build();
        webview.set_settings(&settings);
        webview.connect_permission_request(|_, request| {
            request.deny();
            true
        });
        webview.connect_create(|_, _| None);
        window.set_child(Some(&webview));
        webview.load_html(HTML, None);

        let ready = Rc::new(Cell::new(false));
        let pending: Rc<std::cell::RefCell<Option<(String, u64, bool)>>> =
            Rc::new(std::cell::RefCell::new(None));
        let ready_changed = ready.clone();
        let pending_changed = pending.clone();
        webview.connect_load_changed(move |webview, event| {
            if event != webkit6::LoadEvent::Finished {
                return;
            }
            ready_changed.set(true);
            let pending = pending_changed.borrow_mut().take();
            if let Some((video_id, position, playing)) = pending {
                apply(webview, &video_id, position, playing);
            }
        });

        Self {
            window,
            webview,
            ready,
            pending,
        }
    }

    pub fn show(&self, video_id: &str, position_ms: u64, playing: bool) {
        self.window.present();
        if self.ready.get() {
            apply(&self.webview, video_id, position_ms, playing);
        } else {
            *self.pending.borrow_mut() = Some((video_id.to_owned(), position_ms, playing));
        }
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }
}

fn apply(webview: &webkit6::WebView, video_id: &str, position_ms: u64, playing: bool) {
    let video_id = serde_json::to_string(video_id).unwrap_or_else(|_| "\"\"".to_owned());
    let script = format!("window.ticonnectApply({video_id}, {position_ms}, {playing});");
    webview.evaluate_javascript(&script, None, None, None::<&gtk::gio::Cancellable>, |_| {});
}
