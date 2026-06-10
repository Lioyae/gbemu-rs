use crate::cartridge::header::CgbSupport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareModel {
    Dmg,
    Cgb,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ModelPreference {
    #[default]
    Auto,
    Dmg,
    Cgb,
}

impl ModelPreference {
    pub fn resolve(self, support: CgbSupport) -> HardwareModel {
        match self {
            Self::Auto => match support {
                CgbSupport::DmgOnly => HardwareModel::Dmg,
                CgbSupport::Compatible | CgbSupport::Required => HardwareModel::Cgb,
            },
            Self::Dmg => HardwareModel::Dmg,
            Self::Cgb => HardwareModel::Cgb,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_selects_model_from_cartridge_support() {
        assert_eq!(
            ModelPreference::Auto.resolve(CgbSupport::DmgOnly),
            HardwareModel::Dmg
        );
        assert_eq!(
            ModelPreference::Auto.resolve(CgbSupport::Compatible),
            HardwareModel::Cgb
        );
        assert_eq!(
            ModelPreference::Auto.resolve(CgbSupport::Required),
            HardwareModel::Cgb
        );
    }

    #[test]
    fn explicit_model_overrides_compatible_cartridge() {
        assert_eq!(
            ModelPreference::Dmg.resolve(CgbSupport::Compatible),
            HardwareModel::Dmg
        );
        assert_eq!(
            ModelPreference::Cgb.resolve(CgbSupport::Compatible),
            HardwareModel::Cgb
        );
    }
}
