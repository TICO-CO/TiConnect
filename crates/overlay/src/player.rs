use std::cell::Cell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4_layer_shell::{Layer, LayerShell};
use webkit6::prelude::*;

const HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><style>
*{box-sizing:border-box}
html,body{margin:0;background:#213639;color:#FFE5B0;font-family:"Istok Web",sans-serif;overflow:hidden;border-radius:20px}
.shell{display:flex;flex-direction:column;border:1px solid #3F5A52;border-radius:20px;overflow:hidden;background:#213639}
.head{display:flex;align-items:center;gap:8px;padding:10px 12px;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:#A9A78A}
.head strong{color:#EDB483;font-weight:700}
.grow{flex:1}
.ignore{display:flex;align-items:center;gap:6px}
.switch{width:32px;height:18px;border-radius:999px;background:#EDB483;position:relative}
.switch:after{content:"";position:absolute;top:2px;right:2px;width:14px;height:14px;border-radius:50%;background:#182B2F}
.close{border:0;background:transparent;color:#A9A78A;font-size:16px;cursor:pointer}
#host{width:100%;aspect-ratio:16/9;background:#2A403C}
#host iframe{width:100%;height:100%;border:0}
.copy{display:flex;flex-direction:column;gap:2px;padding:10px 12px 2px}
.copy strong{font-size:15px;font-weight:700;color:#FFE5B0}
.copy span{font-size:11px;letter-spacing:.04em;text-transform:uppercase;color:#A9A78A}
.controls{display:flex;align-items:center;gap:8px;padding:8px 12px 12px}
button{font:inherit;cursor:pointer}
.ctrl{width:28px;height:28px;border:0;border-radius:50%;background:transparent;color:#EDB483;font-size:13px}
.ctrl:hover{background:#2A403C}
.time{font-size:11px;color:#A9A78A;font-variant-numeric:tabular-nums}
.bar{height:3px;flex:1;border-radius:999px;background:#2F4547;overflow:hidden}
.bar i{display:block;height:100%;width:33%;background:#EDB483;border-radius:999px}
</style></head><body><div class="shell">
<div class="head"><strong>YouTube</strong><span class="grow"></span><span class="ignore">Ignore incoming <span class="switch"></span></span><button class="close" onclick="window.close()">×</button></div>
<div id="host"></div>
<div class="copy"><strong id="title">TiConnect video</strong><span id="meta">TiConnect Labs · LAN</span></div>
<div class="controls"><button class="ctrl" onclick="command('playVideo')">▶</button><button class="ctrl" onclick="command('pauseVideo')">❚❚</button><span class="time" id="pos">00:00</span><span class="bar"><i id="progress"></i></span><span class="time">18:42</span><button class="ctrl" onclick="command('mute')">◔</button><button class="ctrl" onclick="command('unMute')">◕</button></div>
</div><script>
let current=null, frame=null, lastPos=0;
function command(func){if(frame&&frame.contentWindow){frame.contentWindow.postMessage(JSON.stringify({event:'command',func:func,args:[]}),'*');}}
function apply(id,pos,playing){
  const position=Math.max(0, Math.floor(pos/1000));
  document.getElementById('title').textContent='TiConnect Labs';
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
  frame.contentWindow.postMessage(JSON.stringify({event:'command',func:playing?'playVideo':'pauseVideo',args:[]}),'*');
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
            .default_width(480)
            .default_height(420)
            .build();
        window.add_css_class("ticonnect-player");
        window.init_layer_shell();
        window.set_anchor(gtk4_layer_shell::Edge::Bottom, true);
        window.set_anchor(gtk4_layer_shell::Edge::Right, true);
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);
        window.set_exclusive_zone(0);
        window.set_margin(gtk4_layer_shell::Edge::Bottom, 24);
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
