use std::cell::RefCell;
use std::os::raw::c_void;
use std::rc::Rc;
use std::sync::{
    mpsc::{channel, Receiver, Sender, TryRecvError},
    Arc, Mutex,
};
use std::thread;

use vst3_sys::{
    base::{char16, kResultFalse, kResultOk, tresult, FIDString, TBool},
    gui::{IPlugFrame, IPlugView, IPlugViewContentScaleSupport, ViewRect},
    utils::SharedVstPtr,
    VST3,
};

use egui_extras::image::RetainedImage;
use egui_glow::{
    egui_winit::{egui, winit},
    glow, EguiGlow,
};
use glutin::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    platform::{
        run_return::EventLoopExtRunReturn,
        unix::{EventLoopBuilderExtUnix, WindowBuilderExtUnix},
    },
    window::WindowBuilder,
    PossiblyCurrent, WindowedContext,
};

use crate::vst3::utils;

const SCREEN_WIDTH: u32 = 680;
const SCREEN_HEIGHT: u32 = 560;

// images
const IMG_LOGO: &[u8] = include_bytes!("../../resources/logo.png");
const IMG_LABEL_GLOBAL: &[u8] = include_bytes!("../../resources/label-global.png");
const IMG_LABEL_SQUARE: &[u8] = include_bytes!("../../resources/label-osc-square.png");
const IMG_LABEL_NOISE: &[u8] = include_bytes!("../../resources/label-osc-noise.png");
const IMG_LABEL_WAVETABLE: &[u8] = include_bytes!("../../resources/label-osc-wavetable.png");
const IMG_LABEL_ENVELOPE: &[u8] = include_bytes!("../../resources/label-envelope.png");
const IMG_LABEL_SWEEP: &[u8] = include_bytes!("../../resources/label-sweep.png");
const IMG_LABEL_STUTTER: &[u8] = include_bytes!("../../resources/label-stutter.png");
const IMG_BUTTON_RESET_RANDOM: &[u8] = include_bytes!("../../resources/button-reset-random.png");
const IMG_BUTTON_RESET_SINE: &[u8] = include_bytes!("../../resources/button-reset-sine.png");
const IMG_SLIDER_BORDER: &[u8] = include_bytes!("../../resources/slider-border.png");
const IMG_VALUE_ATLAS: &[u8] = include_bytes!("../../resources/paramval.png");

mod widget {
    use std::rc::Rc;
    use std::time;

    use egui_extras::image::RetainedImage;
    use egui_glow::egui_winit::{egui, egui::Widget};
    use num;

    #[derive(Clone, Debug)]
    pub struct Toggle {
        value: bool,
        prev_value: bool,
    }

    impl Toggle {
        fn new(v: bool, prev: bool) -> Self {
            Self {
                value: v,
                prev_value: prev,
            }
        }

        fn val(&self) -> bool {
            self.value
        }

        fn set(&mut self, v: bool) {
            self.prev_value = self.value;
            self.value = v;
        }

        fn toggled(&self) -> bool {
            self.value != self.prev_value
        }
    }

    pub trait Behavior {
        fn update(&mut self) -> bool;
        fn show(&mut self, ui: &mut egui::Ui) -> egui::Response;
        fn rect(&self) -> egui::Rect;
    }

    // available characters in resources/paramval.png
    enum Character {
        Digit0,
        Digit1,
        Digit2,
        Digit3,
        Digit4,
        Digit5,
        Digit6,
        Digit7,
        Digit8,
        Digit9,
        Dot,
        Minus,
    }

    type Region = (egui::Vec2, egui::Vec2);

    impl Character {
        fn from_char(ch: char) -> Option<Character> {
            match ch {
                '0' => Some(Character::Digit0),
                '1' => Some(Character::Digit1),
                '2' => Some(Character::Digit2),
                '3' => Some(Character::Digit3),
                '4' => Some(Character::Digit4),
                '5' => Some(Character::Digit5),
                '6' => Some(Character::Digit6),
                '7' => Some(Character::Digit7),
                '8' => Some(Character::Digit8),
                '9' => Some(Character::Digit9),
                '.' => Some(Character::Dot),
                '-' => Some(Character::Minus),
                _ => None,
            }
        }

        fn get_region(&self) -> Region {
            match self {
                Character::Digit0 => (egui::vec2(0.0, 0.0), egui::vec2(10.0, 14.0)),
                Character::Digit1 => (egui::vec2(14.0, 0.0), egui::vec2(6.0, 14.0)),
                Character::Digit2 => (egui::vec2(24.0, 0.0), egui::vec2(10.0, 14.0)),
                Character::Digit3 => (egui::vec2(36.0, 0.0), egui::vec2(10.0, 14.0)),
                Character::Digit4 => (egui::vec2(48.0, 0.0), egui::vec2(10.0, 14.0)),
                Character::Digit5 => (egui::vec2(60.0, 0.0), egui::vec2(10.0, 14.0)),
                Character::Digit6 => (egui::vec2(72.0, 0.0), egui::vec2(10.0, 14.0)),
                Character::Digit7 => (egui::vec2(84.0, 0.0), egui::vec2(10.0, 14.0)),
                Character::Digit8 => (egui::vec2(96.0, 0.0), egui::vec2(10.0, 14.0)),
                Character::Digit9 => (egui::vec2(108.0, 0.0), egui::vec2(10.0, 14.0)),
                Character::Dot => (egui::vec2(132.0, 0.0), egui::vec2(2.0, 14.0)),
                Character::Minus => (egui::vec2(136.0, 0.0), egui::vec2(10.0, 14.0)),
            }
        }
    }

    // available units in resources/paramval.png
    pub enum ParameterUnit {
        None,
        Decibel,
        Cent,
        MilliSec,
        Sec,
        Percent,
    }

    impl ParameterUnit {
        fn get_region(&self) -> Option<Region> {
            match self {
                ParameterUnit::None => None,
                ParameterUnit::Decibel => Some((egui::vec2(0.0, 16.0), egui::vec2(22.0, 14.0))),
                ParameterUnit::Cent => Some((egui::vec2(30.0, 16.0), egui::vec2(58.0, 14.0))),
                ParameterUnit::MilliSec => Some((egui::vec2(96.0, 16.0), egui::vec2(22.0, 14.0))),
                ParameterUnit::Sec => Some((egui::vec2(126.0, 16.0), egui::vec2(10.0, 14.0))),
                ParameterUnit::Percent => Some((egui::vec2(144.0, 16.0), egui::vec2(10.0, 14.0))),
            }
        }
    }

    pub struct ParameterValue {
        atlas: Rc<RetainedImage>,
        regions: Vec<Region>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    }

    impl ParameterValue {
        pub fn new(
            value: f64,
            unit: ParameterUnit,
            formatter: Box<dyn Fn(f64) -> String>,
            atlas: Rc<RetainedImage>,
            x: f32,
            y: f32,
        ) -> Self {
            let (regions, w, h) = ParameterValue::layout(value, unit, formatter);
            Self {
                atlas,
                regions,
                x,
                y,
                w: w,
                h: h,
            }
        }

        pub fn rect(&self) -> egui::Rect {
            let top_left = egui::pos2(self.x, self.y);
            egui::Rect {
                min: top_left,
                max: top_left + egui::vec2(self.w, self.h),
            }
        }

        fn layout(
            value: f64,
            unit: ParameterUnit,
            formatter: Box<dyn Fn(f64) -> String>,
        ) -> (Vec<Region>, f32, f32) {
            let s = (formatter)(value);
            let mut regions = Vec::new();
            let (mut w, mut h) = (0.0, 0.0);

            println!("layout a value {} formatted as {}", value, s);
            for ch in s.chars() {
                match Character::from_char(ch) {
                    Some(c) => {
                        let region = c.get_region();
                        w += region.1.x;
                        h = region.1.y;
                        regions.push(region);
                    }
                    None => {
                        println!("invalid char in the target: '{}'", ch);
                    }
                }
            }

            if let Some(region) = unit.get_region() {
                w += region.1.x;
                h = region.1.y;
                regions.push(region);
            }

            (regions, w, h)
        }
    }

    impl Widget for ParameterValue {
        fn ui(self, ui: &mut egui::Ui) -> egui::Response {
            let rect = egui::Rect {
                min: egui::pos2(self.x, self.y),
                max: egui::pos2(self.x + self.w as f32, self.y + self.w as f32),
            };

            let response = ui.allocate_rect(rect, egui::Sense::focusable_noninteractive());

            if ui.is_rect_visible(rect) {
                let atlas_size = self.atlas.size();
                let atlas_size = egui::vec2(atlas_size[0] as f32, atlas_size[1] as f32);
                let top_left = egui::pos2(self.x, self.y);
                let mut char_offset_x = 0.0;

                for region in self.regions.iter() {
                    let clip_rect = egui::Rect {
                        min: top_left,
                        max: top_left + region.1.into(),
                    };
                    ui.set_clip_rect(clip_rect.translate(egui::vec2(char_offset_x, 0.0)));

                    let draw_rect = egui::Rect {
                        min: top_left,
                        max: top_left + atlas_size.into(),
                    };

                    let img =
                        egui::widgets::Image::new(self.atlas.texture_id(ui.ctx()), atlas_size);
                    img.paint_at(ui, draw_rect.translate(-region.0));

                    char_offset_x += region.1.x + 2.0;
                }
            }

            response
        }
    }

    #[derive(Clone)]
    pub struct ImageLabel {
        image: Rc<RetainedImage>,
        sense: egui::Sense,
        x: f32,
        y: f32,
    }

    impl ImageLabel {
        pub fn new(image: Rc<RetainedImage>, x: f32, y: f32) -> Self {
            Self {
                image: image,
                sense: egui::Sense::focusable_noninteractive(),
                x: x,
                y: y,
            }
        }

        pub fn rect(&self) -> egui::Rect {
            let size = self.image.size();
            egui::Rect {
                min: egui::pos2(self.x, self.y),
                max: egui::pos2(self.x + size[0] as f32, self.y + size[1] as f32),
            }
        }
    }

    impl Widget for ImageLabel {
        fn ui(self, ui: &mut egui::Ui) -> egui::Response {
            let rect = self.rect();

            let response = ui.allocate_rect(rect, self.sense);

            if ui.is_rect_visible(rect) {
                let img = egui::widgets::Image::new(self.image.texture_id(ui.ctx()), rect.size());
                img.paint_at(ui, rect);
            }

            response
        }
    }

    #[derive(Clone)]
    pub struct Button {
        image: Rc<RetainedImage>,
        sense: egui::Sense,
        clicked: bool,
        rect: egui::Rect,
    }

    impl Button {
        pub fn new(image: Rc<RetainedImage>, clicked: bool, rect: egui::Rect) -> Self {
            Self {
                image: image,
                sense: egui::Sense::click().union(egui::Sense::hover()),
                clicked: clicked,
                rect: rect,
            }
        }
    }

    impl Widget for &mut Button {
        fn ui(self, ui: &mut egui::Ui) -> egui::Response {
            let rect = if self.clicked {
                self.rect.translate(egui::vec2(2.0, 2.0))
            } else {
                self.rect
            };

            let response = ui.allocate_rect(rect, self.sense);

            if ui.is_rect_visible(rect) {
                let img = egui::widgets::Image::new(self.image.texture_id(ui.ctx()), rect.size());
                img.paint_at(ui, rect);

                if response.hovered() {
                    ui.painter().rect_filled(
                        rect,
                        egui::Rounding::none(),
                        egui::Color32::from_rgba_unmultiplied(0xab, 0xbb, 0xa8, 80),
                    );
                }
            }

            response
        }
    }

    #[derive(Clone)]
    pub struct ButtonBehavior {
        image: Rc<RetainedImage>,
        clicked_at: time::Instant,
        clicked: Toggle,
        x: f32,
        y: f32,
    }

    impl ButtonBehavior {
        pub fn new(image: Rc<RetainedImage>, x: f32, y: f32) -> Self {
            Self {
                image: image,
                clicked_at: time::Instant::now(),
                clicked: Toggle::new(false, false),
                x: x,
                y: y,
            }
        }
    }

    impl Behavior for ButtonBehavior {
        fn rect(&self) -> egui::Rect {
            let size = self.image.size();
            egui::Rect {
                min: egui::pos2(self.x, self.y),
                max: egui::pos2(self.x + size[0] as f32, self.y + size[1] as f32),
            }
        }

        fn update(&mut self) -> bool {
            if self.clicked_at.elapsed() <= time::Duration::from_millis(100) {
                self.clicked.set(true);
            } else {
                self.clicked.set(false);
            }

            self.clicked.toggled()
        }

        fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
            let mut widget = Button::new(self.image.clone(), self.clicked.val(), self.rect());
            let response = widget.ui(ui);

            if response.clicked() {
                self.clicked_at = time::Instant::now();
            }

            response
        }
    }

    pub struct Slider {
        border_img: Rc<RetainedImage>,
        sense: egui::Sense,
        rect: egui::Rect,
        bipolar: bool,
        value: f64,
    }

    impl Slider {
        pub fn new(
            border_img: Rc<RetainedImage>,
            value: f64,
            bipolar: bool,
            rect: egui::Rect,
        ) -> Self {
            Self {
                border_img: border_img,
                sense: egui::Sense::drag(),
                rect: rect,
                bipolar: bipolar,
                value: value,
            }
        }
    }

    impl Widget for Slider {
        fn ui(self, ui: &mut egui::Ui) -> egui::Response {
            let rect_label = self.rect.clone();
            let _ = ui.allocate_rect(
                rect_label,
                egui::Sense {
                    click: false,
                    drag: false,
                    focusable: false,
                },
            );

            if ui.is_rect_visible(rect_label) {}

            let rect_slider = self.rect.clone().translate(egui::vec2(0.0, 8.0));
            let response = ui.allocate_rect(rect_slider, self.sense);

            if ui.is_rect_visible(rect_slider) {
                let w = self.rect.max.x - 2.0 - self.rect.min.x + 2.0;

                if self.bipolar {
                    if self.value < 0.5 {
                    } else {
                    }
                } else {
                    ui.painter().rect_filled(
                        egui::Rect {
                            min: self.rect.min,
                            max: egui::pos2(
                                self.rect.min.x + w * self.value as f32,
                                self.rect.max.y,
                            ),
                        },
                        egui::Rounding::none(),
                        egui::Color32::from_rgb(0x33, 0x3f, 0x32),
                    );
                }

                let img = egui::widgets::Image::new(
                    self.border_img.texture_id(ui.ctx()),
                    self.rect.size(),
                );
                img.paint_at(ui, self.rect);
            }

            response
        }
    }

    pub struct SliderBehavior {
        border_img: Rc<RetainedImage>,
        bipolar: bool,
        value: f64,
        x: f32,
        y: f32,
    }

    impl SliderBehavior {
        pub fn new(
            border_img: Rc<RetainedImage>,
            value: f64,
            bipolar: bool,
            x: f32,
            y: f32,
        ) -> Self {
            Self {
                border_img: border_img,
                value: value,
                bipolar: bipolar,
                x: x,
                y: y,
            }
        }
    }

    impl Behavior for SliderBehavior {
        fn update(&mut self) -> bool {
            false
        }

        fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
            let widget = Slider::new(
                self.border_img.clone(),
                self.value,
                self.bipolar,
                self.rect(),
            );
            let response = ui.add(widget);

            if response.dragged() {
                let delta_factor = if ui.input().modifiers.shift {
                    // It may be wrong this way...
                    3000.0
                } else {
                    300.0
                };

                let delta_x = response.drag_delta().x;
                let delta_v = delta_x as f64 / delta_factor;
                self.value = num::clamp(self.value + delta_v, 0.0, 1.0);
            }

            response
        }

        fn rect(&self) -> egui::Rect {
            let size = self.border_img.size();
            egui::Rect::from_two_pos(
                egui::pos2(self.x, self.y),
                egui::pos2(self.x + size[0] as f32, self.y + size[1] as f32),
            )
        }
    }
}
use widget::*;

enum GUIMessage {
    Terminate,
}

enum GUIEvent {
    Redraw,
}

struct ParentWindow(*mut c_void);
unsafe impl Send for ParentWindow {}
unsafe impl Sync for ParentWindow {}

struct GUIThread {
    // SoyBoy resources
    atlas_values: Rc<RetainedImage>,
    label_logo: ImageLabel,
    label_global: ImageLabel,
    label_square: ImageLabel,
    label_noise: ImageLabel,
    label_wavetable: ImageLabel,
    label_envelope: ImageLabel,
    label_sweep: ImageLabel,
    label_stutter: ImageLabel,
    button_reset_random: ButtonBehavior,
    button_reset_sine: ButtonBehavior,
    slider_volume: SliderBehavior,
    // window stuff
    quit: bool,
    needs_repaint: bool,
    // threading stuff
    receiver: Arc<Mutex<Receiver<GUIMessage>>>,
    // egui stuff
    egui_glow: EguiGlow,
    window: WindowedContext<PossiblyCurrent>,
    // glow_context: Rc<glow::Context>,
}

// originally from here:
//   https://github.com/emilk/egui/blob/7cd285ecbc2d319f1feac7b9fd9464d06a5ccf77/egui_glow/examples/pure_glow.rs
impl GUIThread {
    fn setup(
        parent: ParentWindow,
        receiver: Arc<Mutex<Receiver<GUIMessage>>>,
    ) -> (Self, EventLoop<GUIEvent>) {
        let parent_id: usize = if parent.0.is_null() {
            0
        } else {
            parent.0 as usize
        };
        let event_loop = EventLoopBuilder::<GUIEvent>::with_user_event()
            .with_any_thread(true)
            .build();

        let window_builder = WindowBuilder::new()
            .with_x11_parent(parent_id.try_into().unwrap())
            .with_resizable(false)
            .with_inner_size(winit::dpi::LogicalSize {
                width: SCREEN_WIDTH as f32,
                height: SCREEN_HEIGHT as f32,
            })
            .with_title("egui_glow example");

        let window = unsafe {
            glutin::ContextBuilder::new()
                .with_depth_buffer(0)
                .with_srgb(true)
                .with_stencil_buffer(0)
                .with_vsync(true)
                .build_windowed(window_builder, &event_loop)
                .unwrap()
                .make_current()
                .unwrap()
        };

        println!("scale factor = {}", window.window().scale_factor());

        let glow_context =
            unsafe { glow::Context::from_loader_function(|s| window.get_proc_address(s)) };
        let glow_context = Rc::new(glow_context);
        let egui_glow = EguiGlow::new(window.window(), glow_context.clone());

        let img_slider_border = Rc::new(
            RetainedImage::from_image_bytes("soyboy:slider:border", IMG_SLIDER_BORDER).unwrap(),
        );

        let thread = GUIThread {
            atlas_values: Rc::new(
                RetainedImage::from_image_bytes("value_atlas", IMG_VALUE_ATLAS).unwrap(),
            ),
            label_logo: ImageLabel::new(
                Rc::new(RetainedImage::from_image_bytes("soyboy:logo", IMG_LOGO).unwrap()),
                6.0,
                6.0,
            ),
            label_global: ImageLabel::new(
                Rc::new(
                    RetainedImage::from_image_bytes("soyboy:label:global", IMG_LABEL_GLOBAL)
                        .unwrap(),
                ),
                24.0,
                86.0,
            ),
            label_square: ImageLabel::new(
                Rc::new(
                    RetainedImage::from_image_bytes("soyboy:label:square", IMG_LABEL_SQUARE)
                        .unwrap(),
                ),
                24.0,
                216.0,
            ),
            label_noise: ImageLabel::new(
                Rc::new(
                    RetainedImage::from_image_bytes("soyboy:label:noise", IMG_LABEL_NOISE).unwrap(),
                ),
                24.0,
                280.0,
            ),
            label_wavetable: ImageLabel::new(
                Rc::new(
                    RetainedImage::from_image_bytes("soyboy:label:wavetable", IMG_LABEL_WAVETABLE)
                        .unwrap(),
                ),
                24.0,
                408.0,
            ),
            label_envelope: ImageLabel::new(
                Rc::new(
                    RetainedImage::from_image_bytes("soyboy:label:envelope", IMG_LABEL_ENVELOPE)
                        .unwrap(),
                ),
                352.0,
                12.0,
            ),
            label_sweep: ImageLabel::new(
                Rc::new(
                    RetainedImage::from_image_bytes("soyboy:label:sweep", IMG_LABEL_SWEEP).unwrap(),
                ),
                352.0,
                184.0,
            ),
            label_stutter: ImageLabel::new(
                Rc::new(
                    RetainedImage::from_image_bytes("soyboy:label:stutter", IMG_LABEL_STUTTER)
                        .unwrap(),
                ),
                352.0,
                316.0,
            ),
            button_reset_random: ButtonBehavior::new(
                Rc::new(
                    RetainedImage::from_image_bytes(
                        "soyboy:button:reset-random",
                        IMG_BUTTON_RESET_RANDOM,
                    )
                    .unwrap(),
                ),
                206.0,
                526.0,
            ),
            button_reset_sine: ButtonBehavior::new(
                Rc::new(
                    RetainedImage::from_image_bytes(
                        "soyboy:button:reset-sine",
                        IMG_BUTTON_RESET_SINE,
                    )
                    .unwrap(),
                ),
                274.0,
                526.0,
            ),
            slider_volume: SliderBehavior::new(img_slider_border, 0.0, false, 60.0, 102.0),
            quit: false,
            needs_repaint: false,
            receiver: receiver,
            egui_glow: egui_glow,
            window: window,
            // glow_context: glow_context,
        };

        (thread, event_loop)
    }

    fn update(&mut self, proxy: EventLoopProxy<GUIEvent>) {
        let mut stateful = [&mut self.button_reset_random, &mut self.button_reset_sine];
        let mut needs_redraw = false;

        for widget in stateful.iter_mut() {
            needs_redraw |= widget.update();
        }

        if needs_redraw {
            let _ = proxy.send_event(GUIEvent::Redraw);
        }
    }

    fn draw(&mut self) {
        self.needs_repaint = self.egui_glow.run(self.window.window(), |egui_ctx| {
            let show_label = |name: &str, label: ImageLabel| {
                let rect = label.rect();
                egui::Area::new(name)
                    .fixed_pos(rect.min)
                    .interactable(false)
                    .show(egui_ctx, |ui| ui.add(label));
            };
            let show_button = |name: &str, button: &mut ButtonBehavior, do_click: &dyn Fn()| {
                let rect = button.rect();
                egui::Area::new(name)
                    .fixed_pos(rect.min)
                    .movable(false)
                    .show(egui_ctx, |ui| {
                        let resp = button.show(ui);
                        if resp.clicked() {
                            do_click();
                        };
                    });
            };
            let show_slider = |name: &str, slider: &mut SliderBehavior| {
                let rect = slider.rect();
                egui::Area::new(name)
                    .fixed_pos(rect.min)
                    .movable(false)
                    .show(egui_ctx, |ui| {
                        let _resp = slider.show(ui);
                    });
            };

            // background
            egui::Area::new("background").show(egui_ctx, |ui| {
                ui.painter().rect_filled(
                    egui::Rect {
                        min: egui::pos2(0.0, 0.0),
                        max: egui::pos2(SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32),
                    },
                    egui::Rounding::none(),
                    egui::Color32::from_rgb(0xab, 0xbb, 0xa8),
                );
            });

            // logo
            show_label("logo", self.label_logo.clone());

            // labels
            {
                // left side
                show_label("label: global", self.label_global.clone());
                show_label("label: square", self.label_square.clone());
                show_label("label: noise", self.label_noise.clone());
                show_label("label: wavetable", self.label_wavetable.clone());

                // right side
                show_label("label: envelope", self.label_envelope.clone());
                show_label("label: sweep", self.label_sweep.clone());
                show_label("label: stutter", self.label_stutter.clone());
            }

            // buttons
            show_button(
                "button: reset wavetable random",
                &mut self.button_reset_random,
                &|| {
                    // TODO: write a code reset plugin's wavetable
                    println!("reset random!!!");
                },
            );
            show_button(
                "button: reset wavetable as sine",
                &mut self.button_reset_sine,
                &|| {
                    // TODO: write a code reset plugin's wavetable
                    println!("reset sine!!!");
                },
            );

            // sliders
            show_slider("slider: test", &mut self.slider_volume);

            // parameter value test
            let paramval = ParameterValue::new(
                10.1502,
                ParameterUnit::Cent,
                Box::new(|v| format!("{}", v)),
                self.atlas_values.clone(),
                100.0,
                200.0,
            );
            let rect = paramval.rect();
            egui::Area::new("test paramval")
                .fixed_pos(rect.min)
                .movable(false)
                .show(egui_ctx, |ui| {
                    let _resp = ui.add(paramval);
                });
        });

        // OpenGL drawing
        {
            self.egui_glow.paint(self.window.window());

            // draw things on top of egui here

            self.window.swap_buffers().unwrap();
        }
    }

    fn proc_events(&mut self, event: Event<GUIEvent>, control_flow: &mut ControlFlow) {
        match self.receiver.lock().unwrap().try_recv() {
            Ok(message) => match message {
                GUIMessage::Terminate => {
                    println!("try_recv() receive Message::Terminate");
                    self.quit = true;
                }
            },
            Err(err) => match err {
                TryRecvError::Empty => {
                    // println!("try_recv() fails because empty");
                }
                TryRecvError::Disconnected => {
                    println!("try_recv() fails because disconnected");
                    self.quit = true;
                }
            },
        }

        let mut redraw = || {
            self.draw();
            if self.needs_repaint {
                self.window.window().request_redraw();
                *control_flow = ControlFlow::Poll;
            } else {
                //*control_flow = ControlFlow::Wait;
                *control_flow = ControlFlow::Poll;
            }
        };

        match event {
            // Platform-dependent event handlers to workaround a winit bug
            // See: https://github.com/rust-windowing/winit/issues/987
            // See: https://github.com/rust-windowing/winit/issues/1619
            Event::RedrawEventsCleared if cfg!(windows) => redraw(),
            Event::RedrawRequested(_) if !cfg!(windows) => redraw(),
            Event::WindowEvent { event, .. } => {
                if matches!(event, WindowEvent::CloseRequested | WindowEvent::Destroyed) {
                    self.quit = true;
                }

                // if let WindowEvent::Resized(physical_size) = &event {
                //     self.window.resize(*physical_size);
                // } else if let WindowEvent::ScaleFactorChanged { new_inner_size, .. } = &event {
                //     self.window.resize(**new_inner_size);
                // }

                self.egui_glow.on_event(&event);
                self.window.window().request_redraw(); // TODO: ask egui if the events warrants a repaint instead
            }
            Event::LoopDestroyed => {
                println!("LoopDestroyed is signaled.");
                self.egui_glow.destroy();
            }
            Event::UserEvent(gui_event) => match gui_event {
                GUIEvent::Redraw => redraw(),
            },
            _ => (),
        }

        if self.quit {
            *control_flow = ControlFlow::Exit;
        }
    }

    fn run_loop(parent: ParentWindow, receiver: Arc<Mutex<Receiver<GUIMessage>>>) {
        let (mut thread, mut event_loop) = GUIThread::setup(parent, receiver);
        let proxy = event_loop.create_proxy();

        event_loop.run_return(move |event, _, control_flow| {
            thread.update(proxy.clone());
            thread.proc_events(event, control_flow);
        });
    }
}

#[VST3(implements(IPlugView, IPlugFrame, IPlugViewContentScaleSupport))]
pub struct SoyBoyGUI {
    scale_factor: RefCell<f32>,
    handle: RefCell<Option<thread::JoinHandle<()>>>,
    sender: RefCell<Option<Sender<GUIMessage>>>,
}

impl SoyBoyGUI {
    pub fn new() -> Box<Self> {
        let scale_factor = RefCell::new(1.0);
        let handle = RefCell::new(None);
        let sender = RefCell::new(None);

        SoyBoyGUI::allocate(scale_factor, handle, sender)
    }

    fn start_gui(&self, parent: ParentWindow) {
        let (send, resv) = channel();
        let recv = Arc::new(Mutex::new(resv));
        (*self.sender.borrow_mut()) = Some(send);

        let handle = thread::spawn(move || {
            GUIThread::run_loop(parent, recv);
        });
        *self.handle.borrow_mut() = Some(handle);
    }
}

impl IPlugFrame for SoyBoyGUI {
    unsafe fn resize_view(
        &self,
        _view: SharedVstPtr<dyn IPlugView>,
        new_size: *mut ViewRect,
    ) -> tresult {
        println!("IPlugFrame::reqise_view()");
        (*new_size).left = 0;
        (*new_size).top = 0;
        (*new_size).right = SCREEN_WIDTH as i32;
        (*new_size).bottom = SCREEN_HEIGHT as i32;

        kResultOk
    }
}

impl IPlugViewContentScaleSupport for SoyBoyGUI {
    unsafe fn set_scale_factor(&self, scale_factor: f32) -> tresult {
        println!(
            "IPlugViewContentScaleSupport::set_scale_factor({})",
            scale_factor
        );
        (*self.scale_factor.borrow_mut()) = scale_factor;
        kResultOk
    }
}

impl IPlugView for SoyBoyGUI {
    unsafe fn is_platform_type_supported(&self, type_: FIDString) -> tresult {
        println!("IPlugView::is_platform_type_supported()");
        let type_ = utils::fidstring_to_string(type_);

        // TODO: currently supports GUI only on GNU/Linux
        if type_ == "X11EmbedWindowID" {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn attached(&self, parent: *mut c_void, type_: FIDString) -> tresult {
        println!("IPlugView::attached()");
        let type_ = utils::fidstring_to_string(type_);

        if type_ == "X11EmbedWindowID" {
            let parent = ParentWindow(parent);
            self.start_gui(parent);
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn removed(&self) -> tresult {
        println!("IPlugView::removed()");
        let old_handle = self.handle.replace(None);
        let _ = (*self.sender.borrow())
            .as_ref()
            .unwrap()
            .send(GUIMessage::Terminate);
        println!("sended terminate.");
        let res = old_handle.unwrap().join();
        println!("joined: {:?}", res);
        let _ = self.sender.replace(None);
        kResultOk
    }
    unsafe fn on_wheel(&self, _distance: f32) -> tresult {
        println!("IPlugView::on_wheel()");
        kResultOk
    }
    unsafe fn on_key_down(&self, _key: char16, _key_code: i16, _modifiers: i16) -> tresult {
        println!("IPlugView::on_key_down()");
        kResultOk
    }
    unsafe fn on_key_up(&self, _key: char16, _key_code: i16, _modifiers: i16) -> tresult {
        println!("IPlugView::on_key_up()");
        kResultOk
    }
    unsafe fn get_size(&self, size: *mut ViewRect) -> tresult {
        println!("IPlugView::get_size()");
        (*size).left = 0;
        (*size).top = 0;
        (*size).right = SCREEN_WIDTH as i32;
        (*size).bottom = SCREEN_HEIGHT as i32;
        kResultOk
    }
    unsafe fn on_size(&self, _new_size: *mut ViewRect) -> tresult {
        println!("IPlugView::on_size()");
        kResultOk
    }
    unsafe fn on_focus(&self, _state: TBool) -> tresult {
        println!("IPlugView::on_focus()");
        kResultOk
    }
    unsafe fn set_frame(&self, frame: *mut c_void) -> tresult {
        println!("IPlugView::set_frame()");
        let frame = frame as *mut _;
        *frame = self as &dyn IPlugFrame;
        kResultOk
    }
    unsafe fn can_resize(&self) -> tresult {
        println!("IPlugView::can_resize()");
        kResultFalse
    }
    unsafe fn check_size_constraint(&self, _rect: *mut ViewRect) -> tresult {
        println!("IPlugView::check_size_constraint()");
        kResultOk
    }
}
