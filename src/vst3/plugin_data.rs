use std::convert::TryFrom;

pub const VST3_VENDOR: &str = "t-sin";
pub const VST3_VERSION: &str = "0.1.0";
pub const VST3_URL: &str = "https://github.com/t-sin/gbi/";
pub const VST3_EMAIL: &str = "shinichi.tanaka45@gmail.com";

pub const VST3_CLASS_NAME: &str = "gbi";
pub const VST3_CLASS_CATEGORY: &str = "Audio Module Class";
pub const VST3_CLASS_SUBCATEGORIES: &str = "Instrument|Synth";

pub const VST3_CID: [u8; 16] = [
    0xd6, 0x8e, 0x5c, 0xd2, 0x8a, 0x5d, 0x4d, 0xbe, 0xaf, 0xfa, 0x4a, 0x3f, 0x01, 0xfc, 0x93, 0xd1,
];

pub const VST3_CONTROLLER_CLASS_NAME: &str = "gbi Controller";
pub const VST3_CONTROLLER_CLASS_CATEGORY: &str = "Component Controller Class";
pub const VST3_CONTROLLER_CLASS_SUBCATEGORIES: &str = "";

pub const VST3_CONTROLLER_CID: [u8; 16] = [
    0x81, 0x24, 0x78, 0x8a, 0x16, 0x37, 0x41, 0xf8, 0x8b, 0xc3, 0x71, 0x07, 0x10, 0x4a, 0x0b, 0x8d,
];

pub enum PluginParameter {
    Param1 = 0,
}

impl TryFrom<u32> for PluginParameter {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == PluginParameter::Param1 as u32 {
            Ok(PluginParameter::Param1)
        } else {
            Err(())
        }
    }
}
