use cairo::{Context, Format, ImageSurface};
use calloop::channel::{channel, Event as ChannelEvent, Sender};
use calloop::timer::{TimeoutAction, Timer};
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::wl_output::{Transform, WlOutput};
use smithay_client_toolkit::reexports::client::protocol::wl_shm::Format as WlShmFormat;
use smithay_client_toolkit::reexports::client::protocol::{
    wl_pointer, wl_seat, wl_surface::WlSurface,
};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm, registry_handlers,
};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use zbus::blocking::connection::Builder;
use zbus::interface;
use zbus::zvariant::Value;

const FONT_FAMILY: &str = "JetBrainsMono Nerd Font";
const FONT_SIZE: f64 = 12.0;
const BODY_FONT_SIZE: f64 = 14.0;
const BG_COLOR: (f64, f64, f64) = (0.12, 0.12, 0.18);
const TEXT_COLOR: (f64, f64, f64) = (0.8, 0.84, 0.96);
const BORDER_COLOR: (f64, f64, f64) = (0.54, 0.71, 0.98);
const BORDER_SIZE: f64 = 2.0;
const PADDING: f64 = 15.0;
const WIDTH: i32 = 360;
const MIN_HEIGHT: i32 = 80;
const MAX_BUFFER_HEIGHT: i32 = 1024;

const MAX_VISIBLE: usize = 5;
const GAP: i32 = 5;
const TOP_MARGIN: i32 = 20;
const RIGHT_MARGIN: i32 = 20;
const TIMEOUT_LOW_NORMAL: Duration = Duration::from_secs(5);
const EXPIRY_SWEEP_INTERVAL: Duration = Duration::from_millis(100);

fn resolve_timeout(expire_timeout: i32, hints: &HashMap<&str, Value<'_>>) -> Option<Duration> {
    if expire_timeout > 0 {
        return Some(Duration::from_millis(expire_timeout as u64));
    }
    if expire_timeout == 0 {
        return None;
    }
    let urgency = hints
        .get("urgency")
        .and_then(|v| v.downcast_ref::<u8>().ok())
        .unwrap_or(1);
    if urgency >= 2 {
        None
    } else {
        Some(TIMEOUT_LOW_NORMAL)
    }
}

fn text_width(cr: &Context, text: &str) -> f64 {
    cr.text_extents(text).map(|e| e.x_advance()).unwrap_or(0.0)
}

fn split_long_word(cr: &Context, word: &str, max_width: f64) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();

    for ch in word.chars() {
        let candidate = format!("{current}{ch}");
        let width = text_width(cr, &candidate);

        if width <= max_width || current.is_empty() {
            current.push(ch);
        } else {
            out.push(std::mem::take(&mut current));
            current.push(ch);
        }
    }

    if !current.is_empty() {
        out.push(current);
    }

    out
}

fn wrap_paragraph(cr: &Context, paragraph: &str, max_width: f64) -> Vec<String> {
    if paragraph.trim().is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for word in paragraph.split_whitespace() {
        if text_width(cr, word) <= max_width {
            if current.is_empty() {
                current = word.to_string();
            } else {
                let candidate = format!("{current} {word}");
                if text_width(cr, &candidate) <= max_width {
                    current = candidate;
                } else {
                    lines.push(std::mem::take(&mut current));
                    current = word.to_string();
                }
            }
        } else {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }

            let chunks = split_long_word(cr, word, max_width);
            if let Some((last, rest)) = chunks.split_last() {
                for chunk in rest {
                    lines.push(chunk.clone());
                }
                current = last.clone();
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn wrap_text(cr: &Context, text: &str, max_width: f64) -> Vec<String> {
    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
        } else {
            lines.extend(wrap_paragraph(cr, paragraph, max_width));
        }
    }

    lines
}

fn configure_text_context(cr: &Context) {
    let mut font_opts = cairo::FontOptions::new().unwrap();
    font_opts.set_antialias(cairo::Antialias::Good);
    font_opts.set_hint_style(cairo::HintStyle::Slight);
    font_opts.set_hint_metrics(cairo::HintMetrics::On);
    cr.set_font_options(&font_opts);
}

const ELLIPSIS: &str = "\u{2026}";

fn append_ellipsis(cr: &Context, line: &str, max_width: f64) -> String {
    if text_width(cr, ELLIPSIS) > max_width {
        return String::new();
    }
    if text_width(cr, &format!("{line}{ELLIPSIS}")) <= max_width {
        return format!("{line}{ELLIPSIS}");
    }
    let mut chars: Vec<char> = line.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect();
        if text_width(cr, &format!("{candidate}{ELLIPSIS}")) <= max_width {
            return format!("{candidate}{ELLIPSIS}");
        }
    }
    ELLIPSIS.to_string()
}

fn fit_lines_to_height(
    cr: &Context,
    summary_lines: Vec<String>,
    body_lines: Vec<String>,
    max_width: f64,
    summary_line_height: f64,
    body_line_height: f64,
    section_gap: f64,
    available_height: f64,
) -> (Vec<String>, Vec<String>) {
    let has_body = !body_lines.is_empty();

    let max_summary_lines = if summary_line_height > 0.0 {
        (available_height / summary_line_height).floor() as usize
    } else {
        summary_lines.len()
    };

    if summary_lines.len() > max_summary_lines {
        if max_summary_lines == 0 {
            return (Vec::new(), Vec::new());
        }
        cr.select_font_face(
            FONT_FAMILY,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Bold,
        );
        cr.set_font_size(FONT_SIZE);
        let mut kept: Vec<String> = summary_lines.into_iter().take(max_summary_lines).collect();
        if let Some(last) = kept.last_mut() {
            *last = append_ellipsis(cr, last, max_width);
        }
        return (kept, Vec::new());
    }

    let used = summary_lines.len() as f64 * summary_line_height;
    let gap = if !summary_lines.is_empty() && has_body {
        section_gap
    } else {
        0.0
    };
    let remaining = available_height - used - gap;

    if !has_body {
        return (summary_lines, Vec::new());
    }

    let max_body_lines = if body_line_height > 0.0 && remaining > 0.0 {
        (remaining / body_line_height).floor() as usize
    } else {
        0
    };

    if body_lines.len() <= max_body_lines {
        return (summary_lines, body_lines);
    }

    cr.select_font_face(
        FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    cr.set_font_size(BODY_FONT_SIZE);

    if max_body_lines == 0 {
        let mut kept_summary = summary_lines;
        cr.select_font_face(
            FONT_FAMILY,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Bold,
        );
        cr.set_font_size(FONT_SIZE);
        if let Some(last) = kept_summary.last_mut() {
            *last = append_ellipsis(cr, last, max_width);
        }
        return (kept_summary, Vec::new());
    }

    let mut kept_body: Vec<String> = body_lines.into_iter().take(max_body_lines).collect();
    if let Some(last) = kept_body.last_mut() {
        *last = append_ellipsis(cr, last, max_width);
    }
    (summary_lines, kept_body)
}

fn measure_layout(summary: &str, body: &str) -> NotificationLayout {
    let surface = ImageSurface::create(Format::ARgb32, 1, 1).unwrap();
    let cr = Context::new(&surface).unwrap();
    configure_text_context(&cr);

    let max_width = WIDTH as f64 - PADDING * 2.0;

    cr.select_font_face(
        FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    cr.set_font_size(FONT_SIZE);
    let summary_lines = wrap_text(&cr, summary, max_width);
    let summary_extents = cr.font_extents().unwrap();

    cr.select_font_face(
        FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    cr.set_font_size(BODY_FONT_SIZE);
    let body_lines = wrap_text(&cr, body, max_width);
    let body_extents = cr.font_extents().unwrap();

    let summary_height = summary_extents.height();
    let body_height = body_extents.height();
    let section_gap = if !summary_lines.is_empty() && !body_lines.is_empty() {
        body_height * 0.45
    } else {
        0.0
    };

    let content_height = if summary_lines.is_empty() {
        body_lines.len() as f64 * body_height
    } else if body_lines.is_empty() {
        summary_lines.len() as f64 * summary_height
    } else {
        summary_lines.len() as f64 * summary_height
            + section_gap
            + body_lines.len() as f64 * body_height
    };

    let available_height = MAX_BUFFER_HEIGHT as f64 - PADDING * 2.0;

    let (summary_lines, body_lines, content_height) = if content_height > available_height {
        let (summary_lines, body_lines) = fit_lines_to_height(
            &cr,
            summary_lines,
            body_lines,
            max_width,
            summary_height,
            body_height,
            section_gap,
            available_height,
        );
        let section_gap = if !summary_lines.is_empty() && !body_lines.is_empty() {
            section_gap
        } else {
            0.0
        };
        let fitted_height = summary_lines.len() as f64 * summary_height
            + section_gap
            + body_lines.len() as f64 * body_height;
        (summary_lines, body_lines, fitted_height)
    } else {
        (summary_lines, body_lines, content_height)
    };

    let height = (PADDING * 2.0 + content_height).ceil() as i32;
    let height = height.max(MIN_HEIGHT).min(MAX_BUFFER_HEIGHT);

    NotificationLayout {
        summary_lines,
        body_lines,
        height,
    }
}

fn top_margin_for_index(active: &[ActiveNotification], index: usize) -> i32 {
    let mut top = TOP_MARGIN;
    for notif in active.iter().take(index) {
        top += notif.layout.height + GAP;
    }
    top
}

#[derive(Clone)]
struct PendingNotification {
    id: u32,
    summary: String,
    body: String,
    timeout: Option<Duration>,
}

struct NotificationLayout {
    summary_lines: Vec<String>,
    body_lines: Vec<String>,
    height: i32,
}

struct ActiveNotification {
    id: u32,
    layout: NotificationLayout,
    expire_at: Option<Instant>,
    layer_surface: LayerSurface,
    configured: bool,
    should_draw: bool,
}

enum DaemonEvent {
    Show(PendingNotification),
    Close(u32),
}

struct NotificationDaemon {
    cmd_tx: Sender<DaemonEvent>,
    next_id: AtomicU32,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationDaemon {
    fn notify(
        &self,
        _app_name: &str,
        _replaces_id: u32,
        _app_icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<&str>,
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let timeout = resolve_timeout(expire_timeout, &hints);
        let _ = self.cmd_tx.send(DaemonEvent::Show(PendingNotification {
            id,
            summary: summary.to_string(),
            body: body.to_string(),
            timeout,
        }));
        id
    }

    fn close_notification(&self, id: u32) {
        let _ = self.cmd_tx.send(DaemonEvent::Close(id));
    }

    fn get_capabilities(&self) -> Vec<&str> {
        vec!["body"]
    }

    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("rust-mako", "ar175", "0.1.0", "1.2")
    }
}

struct AppState {
    qh: QueueHandle<AppState>,
    registry_state: RegistryState,
    compositor_state: CompositorState,
    output_state: OutputState,
    seat_state: SeatState,
    layer_shell: LayerShell,
    shm_state: Shm,
    pool: SlotPool,
    pointer: Option<wl_pointer::WlPointer>,
    active: Vec<ActiveNotification>,
    pending: VecDeque<PendingNotification>,
    clicked: Vec<WlSurface>,
}

impl AppState {
    fn enqueue_notification(&mut self, notif: PendingNotification) {
        if self.active.len() >= MAX_VISIBLE {
            self.pending.push_back(notif);
        } else {
            self.spawn_notification(notif);
        }
    }

    fn spawn_notification(&mut self, notif: PendingNotification) {
        let layout = measure_layout(&notif.summary, &notif.body);
        let surface = self.compositor_state.create_surface(&self.qh);
        let l_surface = self.layer_shell.create_layer_surface(
            &self.qh,
            surface,
            Layer::Overlay,
            Some("notification"),
            None,
        );

        l_surface.set_size(WIDTH as u32, layout.height as u32);
        l_surface.set_anchor(Anchor::TOP | Anchor::RIGHT);
        l_surface.set_margin(
            top_margin_for_index(&self.active, self.active.len()),
            RIGHT_MARGIN,
            0,
            0,
        );
        l_surface.commit();

        self.active.push(ActiveNotification {
            id: notif.id,
            layout,
            expire_at: notif.timeout.map(|d| Instant::now() + d),
            layer_surface: l_surface,
            configured: false,
            should_draw: true,
        });
    }

    fn close_notification_by_id(&mut self, id: u32) {
        if let Some(index) = self.active.iter().position(|n| n.id == id) {
            self.dismiss(index);
            return;
        }

        let mut new_pending = VecDeque::new();
        while let Some(notif) = self.pending.pop_front() {
            if notif.id != id {
                new_pending.push_back(notif);
            }
        }
        self.pending = new_pending;
    }

    fn dismiss(&mut self, index: usize) {
        if index >= self.active.len() {
            return;
        }

        self.active.remove(index);
        self.reflow();

        while self.active.len() < MAX_VISIBLE {
            let Some(next) = self.pending.pop_front() else {
                break;
            };
            self.spawn_notification(next);
        }
    }

    fn reflow(&mut self) {
        for i in 0..self.active.len() {
            if !self.active[i].configured {
                continue;
            }
            let top = top_margin_for_index(&self.active, i);
            self.active[i]
                .layer_surface
                .set_margin(top, RIGHT_MARGIN, 0, 0);
            self.active[i].layer_surface.commit();
        }
    }

    fn sweep_expired(&mut self) {
        let now = Instant::now();
        while let Some(i) = self
            .active
            .iter()
            .position(|n| n.expire_at.is_some_and(|t| t <= now))
        {
            self.dismiss(i);
        }
    }

    fn draw(&mut self, index: usize) {
        let (summary_lines, body_lines, height) = {
            let Some(notif) = self.active.get(index) else {
                return;
            };
            if !notif.configured {
                return;
            }
            (
                notif.layout.summary_lines.clone(),
                notif.layout.body_lines.clone(),
                notif.layout.height,
            )
        };

        let (buffer, canvas) = self
            .pool
            .create_buffer(WIDTH, height, WIDTH * 4, WlShmFormat::Argb8888)
            .expect("Failed to create buffer");

        let mut local_surface = ImageSurface::create(Format::ARgb32, WIDTH, height).unwrap();
        let cr = Context::new(&local_surface).unwrap();
        configure_text_context(&cr);

        cr.set_source_rgb(BG_COLOR.0, BG_COLOR.1, BG_COLOR.2);
        cr.paint().unwrap();

        let half_border = BORDER_SIZE / 2.0;
        cr.set_source_rgb(BORDER_COLOR.0, BORDER_COLOR.1, BORDER_COLOR.2);
        cr.set_line_width(BORDER_SIZE);
        cr.rectangle(
            half_border,
            half_border,
            WIDTH as f64 - BORDER_SIZE,
            height as f64 - BORDER_SIZE,
        );
        cr.stroke().unwrap();

        cr.set_source_rgb(TEXT_COLOR.0, TEXT_COLOR.1, TEXT_COLOR.2);

        cr.select_font_face(
            FONT_FAMILY,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Bold,
        );
        cr.set_font_size(FONT_SIZE);
        let summary_extents = cr.font_extents().unwrap();
        let mut y = PADDING + summary_extents.ascent();

        for line in &summary_lines {
            cr.move_to(PADDING, y);
            cr.show_text(line).unwrap();
            y += summary_extents.height();
        }

        cr.select_font_face(
            FONT_FAMILY,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
        );
        cr.set_font_size(BODY_FONT_SIZE);
        let body_extents = cr.font_extents().unwrap();

        if !body_lines.is_empty() {
            if !summary_lines.is_empty() {
                y += body_extents.height() * 0.45;
            }
            y += body_extents.ascent();
            for line in &body_lines {
                cr.move_to(PADDING, y);
                cr.show_text(line).unwrap();
                y += body_extents.height();
            }
        }

        drop(cr);
        let data = local_surface.data().unwrap();
        canvas[..data.len()].copy_from_slice(&data);

        {
            let notif = &mut self.active[index];
            let surface = notif.layer_surface.wl_surface();
            surface.damage_buffer(0, 0, WIDTH, height);
            buffer.attach_to(surface).expect("buffer attach");
            notif.layer_surface.commit();
            notif.should_draw = false;
        }
    }
}

fn main() {
    let (cmd_tx, cmd_rx) = channel::<DaemonEvent>();

    std::thread::spawn(move || {
        let daemon = NotificationDaemon {
            cmd_tx,
            next_id: AtomicU32::new(1),
        };

        let result = Builder::session()
            .and_then(|b| b.name("org.freedesktop.Notifications"))
            .and_then(|b| b.serve_at("/org/freedesktop/Notifications", daemon))
            .and_then(|b| b.build());

        match result {
            Ok(_conn) => loop {
                std::thread::park();
            },
            Err(e) => {
                eprintln!(
                    "mako-rs: failed to register org.freedesktop.Notifications on the session bus: {e}"
                );
                std::process::exit(1);
            }
        }
    });

    let conn = Connection::connect_to_env().unwrap();
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let shm = Shm::bind(&globals, &qh).unwrap();
    let pool = SlotPool::new(
        WIDTH as usize * MAX_BUFFER_HEIGHT as usize * 4 * MAX_VISIBLE,
        &shm,
    )
    .unwrap();

    let mut app_state = AppState {
        qh: qh.clone(),
        registry_state: RegistryState::new(&globals),
        compositor_state: CompositorState::bind(&globals, &qh).unwrap(),
        output_state: OutputState::new(&globals, &qh),
        seat_state: SeatState::new(&globals, &qh),
        layer_shell: LayerShell::bind(&globals, &qh).unwrap(),
        shm_state: shm,
        pool,
        pointer: None,
        active: Vec::new(),
        pending: VecDeque::new(),
        clicked: Vec::new(),
    };

    let mut event_loop: EventLoop<AppState> = EventLoop::try_new().unwrap();
    let handle = event_loop.handle();

    WaylandSource::new(conn, event_queue)
        .insert(handle.clone())
        .expect("failed to insert wayland source");

    handle
        .insert_source(cmd_rx, |event, _, state: &mut AppState| {
            if let ChannelEvent::Msg(msg) = event {
                match msg {
                    DaemonEvent::Show(notif) => state.enqueue_notification(notif),
                    DaemonEvent::Close(id) => state.close_notification_by_id(id),
                }
            }
        })
        .expect("failed to insert notification channel");

    handle
        .insert_source(
            Timer::from_duration(EXPIRY_SWEEP_INTERVAL),
            |_deadline, _, state: &mut AppState| {
                state.sweep_expired();
                TimeoutAction::ToDuration(EXPIRY_SWEEP_INTERVAL)
            },
        )
        .expect("failed to insert expiry timer");

    event_loop
        .run(None, &mut app_state, |state| {
            for surface in std::mem::take(&mut state.clicked) {
                if let Some(i) = state
                    .active
                    .iter()
                    .position(|n| *n.layer_surface.wl_surface() == surface)
                {
                    state.dismiss(i);
                }
            }
        })
        .expect("event loop failed");
}

delegate_registry!(AppState);
delegate_compositor!(AppState);
delegate_output!(AppState);
delegate_layer!(AppState);
delegate_shm!(AppState);
delegate_seat!(AppState);
delegate_pointer!(AppState);

impl CompositorHandler for AppState {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: u32) {}

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &WlOutput,
    ) {
    }
}

impl OutputHandler for AppState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
}

impl LayerShellHandler for AppState {
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &LayerSurface,
        _configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        let Some(i) = self.active.iter().position(|n| n.layer_surface == *surface) else {
            return;
        };
        self.active[i].configured = true;
        if self.active[i].should_draw {
            self.draw(i);
        }
    }

    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &LayerSurface) {
        if let Some(i) = self.active.iter().position(|n| n.layer_surface == *surface) {
            self.dismiss(i);
        }
    }
}

impl ShmHandler for AppState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm_state
    }
}

impl SeatHandler for AppState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = Some(
                self.seat_state
                    .get_pointer(qh, &seat)
                    .expect("Failed to create pointer"),
            );
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for AppState {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if matches!(event.kind, PointerEventKind::Press { .. }) {
                self.clicked.push(event.surface.clone());
            }
        }
    }
}

impl ProvidesRegistryState for AppState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState, SeatState);
}
