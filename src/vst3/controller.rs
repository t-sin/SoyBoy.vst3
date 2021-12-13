use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::mem;

use std::os::raw::c_void;
use std::ptr::null_mut;

use vst3_com::sys::GUID;
use vst3_sys::{
    base::{
        char16, kInvalidArgument, kResultFalse, kResultOk, kResultTrue, tresult, FIDString,
        IBStream, IPluginBase, TBool,
    },
    gui::{IPlugView, ViewRect},
    utils::SharedVstPtr,
    vst::{
        kRootUnitId, CtrlNumber, IComponentHandler, IEditController, IMidiMapping, IUnitInfo,
        ParamID, ParameterFlags, ParameterInfo, ProgramListInfo, TChar, UnitInfo,
    },
    VST3,
};

use crate::soyboy::parameters::{Normalizable, Parameter, SoyBoyParameter};
use crate::vst3::{plugin_data, utils};

#[VST3(implements(IEditController, IUnitInfo, IMidiMapping, IPlugView))]
pub struct SoyBoyController {
    soyboy_params: HashMap<Parameter, SoyBoyParameter>,
    vst3_params: RefCell<HashMap<u32, ParameterInfo>>,
    param_values: RefCell<HashMap<u32, f64>>,
}

impl SoyBoyController {
    pub const CID: GUID = GUID {
        data: plugin_data::VST3_CONTROLLER_CID,
    };

    unsafe fn add_parameter(
        &self,
        id: u32,
        title: &str,
        short_title: &str,
        units: &str,
        step_count: i32,
        default_value: f64,
        flags: i32,
    ) {
        let mut vst3_params = self.vst3_params.borrow_mut();
        let mut param_vals = self.param_values.borrow_mut();

        let mut param = utils::make_empty_param_info();
        param.id = id;
        utils::wstrcpy(title, param.title.as_mut_ptr());
        utils::wstrcpy(short_title, param.short_title.as_mut_ptr());
        utils::wstrcpy(units, param.units.as_mut_ptr());
        param.step_count = step_count;
        param.default_normalized_value = default_value;
        param.unit_id = kRootUnitId;
        param.flags = flags;

        (*vst3_params).insert(id, param);
        (*param_vals).insert(id, param.default_normalized_value);
    }

    pub unsafe fn new(soyboy_params: HashMap<Parameter, SoyBoyParameter>) -> Box<SoyBoyController> {
        let vst3_params = RefCell::new(HashMap::new());
        let param_vals = RefCell::new(HashMap::new());

        SoyBoyController::allocate(soyboy_params, vst3_params, param_vals)
    }
}

impl IPluginBase for SoyBoyController {
    unsafe fn initialize(&self, _host_context: *mut c_void) -> tresult {
        let soyboy_params = self.soyboy_params.clone();
        for (param, soyboy_param) in soyboy_params.iter() {
            self.add_parameter(
                *param as u32,
                &soyboy_param.title,
                &soyboy_param.short_title,
                &soyboy_param.unit_name,
                soyboy_param.step_count,
                soyboy_param.default_value,
                ParameterFlags::kCanAutomate as i32,
            );
        }

        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IMidiMapping for SoyBoyController {
    unsafe fn get_midi_controller_assignment(
        &self,
        _bus_index: i32,
        _channel: i16,
        midi_cc_number: CtrlNumber,
        param_id: *mut ParamID,
    ) -> tresult {
        match midi_cc_number {
            // kPitchBend
            // cf.
            // - https://www.utsbox.com/?p=1109
            // - https://steinbergmedia.github.io/vst3_doc/vstinterfaces/namespaceSteinberg_1_1Vst.html#a70ee68a13248febed5047cfa0fddf4e6
            129 => {
                *param_id = Parameter::PitchBend as u32;
                kResultTrue
            }
            _ => kResultFalse,
        }
    }
}

impl IEditController for SoyBoyController {
    unsafe fn set_component_state(&self, state: SharedVstPtr<dyn IBStream>) -> tresult {
        if state.is_null() {
            return kResultFalse;
        }

        let state = state.upgrade();
        if state.is_none() {
            return kResultFalse;
        }
        let state = state.unwrap();

        let mut num_bytes_read = 0;
        for param in Parameter::iter() {
            let mut value = 0.0;
            let ptr = &mut value as *mut f64 as *mut c_void;

            state.read(ptr, mem::size_of::<f64>() as i32, &mut num_bytes_read);
            self.param_values.borrow_mut().insert(param as u32, value);
        }

        kResultOk
    }

    unsafe fn set_state(&self, _state: SharedVstPtr<dyn IBStream>) -> tresult {
        kResultOk
    }

    unsafe fn get_state(&self, _state: SharedVstPtr<dyn IBStream>) -> tresult {
        kResultOk
    }

    unsafe fn get_parameter_count(&self) -> i32 {
        self.vst3_params.borrow().len() as i32
    }

    unsafe fn get_parameter_info(&self, id: i32, vst3_params: *mut ParameterInfo) -> tresult {
        let id = id as u32;

        if let Some(param) = self.vst3_params.borrow().get(&id) {
            *vst3_params = *param;

            kResultOk
        } else {
            kInvalidArgument
        }
    }

    unsafe fn get_param_string_by_value(
        &self,
        id: u32,
        value_normalized: f64,
        string: *mut TChar,
    ) -> tresult {
        match Parameter::try_from(id) {
            Ok(param) => {
                if let Some(p) = self.soyboy_params.get(&param) {
                    utils::tcharcpy(&p.format(value_normalized), string)
                } else {
                    return kResultFalse;
                }
            }
            _ => (),
        }

        kResultOk
    }

    unsafe fn get_param_value_by_string(
        &self,
        id: u32,
        string: *const TChar,
        value_normalized: *mut f64,
    ) -> tresult {
        match Parameter::try_from(id) {
            Ok(param) => {
                if let Some(p) = self.soyboy_params.get(&param) {
                    if let Some(v) = p.parse(&utils::tchar_to_string(string)) {
                        *value_normalized = v;
                    } else {
                        return kResultFalse;
                    }
                } else {
                    return kResultFalse;
                }
            }
            _ => (),
        }
        kResultOk
    }

    unsafe fn normalized_param_to_plain(&self, id: u32, value_normalized: f64) -> f64 {
        match Parameter::try_from(id) {
            Ok(param) => {
                if let Some(p) = self.soyboy_params.get(&param) {
                    p.denormalize(value_normalized)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    unsafe fn plain_param_to_normalized(&self, id: u32, value_plain: f64) -> f64 {
        match Parameter::try_from(id) {
            Ok(param) => {
                if let Some(p) = self.soyboy_params.get(&param) {
                    p.normalize(value_plain)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    unsafe fn get_param_normalized(&self, id: u32) -> f64 {
        match self.param_values.borrow().get(&id) {
            Some(val) => *val,
            _ => 0.0,
        }
    }

    unsafe fn set_param_normalized(&self, id: u32, value: f64) -> tresult {
        match self.param_values.borrow_mut().insert(id, value) {
            Some(_) => kResultTrue,
            _ => kResultFalse,
        }
    }

    unsafe fn set_component_handler(
        &self,
        _handler: SharedVstPtr<dyn IComponentHandler>,
    ) -> tresult {
        kResultOk
    }

    unsafe fn create_view(&self, name: FIDString) -> *mut c_void {
        if utils::fidstring_to_string(name) == "editor" {
            self as &dyn IPlugView as *const dyn IPlugView as *mut c_void
        } else {
            null_mut()
        }
    }
}

impl IUnitInfo for SoyBoyController {
    unsafe fn get_unit_count(&self) -> i32 {
        1
    }

    unsafe fn get_unit_info(&self, _unit_index: i32, _info: *mut UnitInfo) -> i32 {
        kResultFalse
    }

    unsafe fn get_program_list_count(&self) -> i32 {
        0
    }

    unsafe fn get_program_list_info(&self, _list_index: i32, _info: *mut ProgramListInfo) -> i32 {
        kResultFalse
    }

    unsafe fn get_program_name(&self, _list_id: i32, _program_index: i32, _name: *mut u16) -> i32 {
        kResultFalse
    }

    unsafe fn get_program_info(
        &self,
        _list_id: i32,
        _program_index: i32,
        _attribute_id: *const u8,
        _attribute_value: *mut u16,
    ) -> i32 {
        kResultFalse
    }

    unsafe fn has_program_pitch_names(&self, _id: i32, _index: i32) -> i32 {
        kResultFalse
    }

    unsafe fn get_program_pitch_name(
        &self,
        _id: i32,
        _index: i32,
        _pitch: i16,
        _name: *mut u16,
    ) -> i32 {
        kResultFalse
    }

    unsafe fn get_selected_unit(&self) -> i32 {
        0
    }

    unsafe fn select_unit(&self, _id: i32) -> i32 {
        kResultFalse
    }

    unsafe fn get_unit_by_bus(
        &self,
        _type_: i32,
        _dir: i32,
        _index: i32,
        _channel: i32,
        _unit_id: *mut i32,
    ) -> i32 {
        kResultFalse
    }

    unsafe fn set_unit_program_data(
        &self,
        _list_or_unit: i32,
        _program_index: i32,
        _data: SharedVstPtr<dyn IBStream>,
    ) -> i32 {
        kResultFalse
    }
}

impl IPlugView for SoyBoyController {
    unsafe fn is_platform_type_supported(&self, type_: FIDString) -> tresult {
        println!("aaaaaaaaaaaaa");
        let type_ = utils::fidstring_to_string(type_);

        // TODO: currently supports GUI only on GNU/Linux
        if type_ == "X11EmbedWindowID" {
            println!("aaaaaaaaaaaaa");
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn attached(&self, _parent: *mut c_void, _type_: FIDString) -> tresult {
        println!("aaaaaaaaaa");
        kResultOk
    }

    unsafe fn removed(&self) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
    unsafe fn on_wheel(&self, _distance: f32) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
    unsafe fn on_key_down(&self, _key: char16, _key_code: i16, _modifiers: i16) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
    unsafe fn on_key_up(&self, _key: char16, _key_code: i16, _modifiers: i16) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
    unsafe fn get_size(&self, _size: *mut ViewRect) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
    unsafe fn on_size(&self, _new_size: *mut ViewRect) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
    unsafe fn on_focus(&self, _state: TBool) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
    unsafe fn set_frame(&self, _frame: *mut c_void) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
    unsafe fn can_resize(&self) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
    unsafe fn check_size_constraint(&self, _rect: *mut ViewRect) -> tresult {
        println!("aaaaaaaaaaaaa");
        kResultOk
    }
}
