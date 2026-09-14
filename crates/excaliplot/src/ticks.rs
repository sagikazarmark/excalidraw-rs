use crate::Error;

/// Bounded numeric display policy, independent of coordinate mapping.
/// Explicit precision is fixed (0..=9 places); indistinguishable rounded ticks
/// return an error. The default trims trailing zeros at seven decimal places.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TickFormat {
    #[default]
    Auto,
    Decimal {
        places: u8,
    },
    /// ASCII `e` notation, with places after the mantissa's decimal point.
    Scientific {
        places: u8,
    },
    /// Multiply fractions by 100 for display only (0.25 becomes 25%).
    FractionPercent {
        places: u8,
    },
    /// Values are already percentages (25 becomes 25%).
    Percent {
        places: u8,
    },
}

impl TickFormat {
    pub(crate) fn validate(self) -> Result<(), Error> {
        let places = match self {
            Self::Auto => 7,
            Self::Decimal { places }
            | Self::Scientific { places }
            | Self::FractionPercent { places }
            | Self::Percent { places } => places,
        };
        if places > 9 {
            return Err(Error::Invalid("tick precision must be 0..=9 places"));
        }
        Ok(())
    }

    pub(crate) fn label(self, value: f64) -> String {
        let (value, places, percent) = match self {
            Self::Auto => (value, 7, false),
            Self::Decimal { places } | Self::Scientific { places } => (value, places, false),
            Self::FractionPercent { places } => (value * 100.0, places, true),
            Self::Percent { places } => (value, places, true),
        };
        let mut label = if matches!(self, Self::Scientific { .. }) {
            format!("{value:.places$e}", places = usize::from(places))
        } else {
            format!("{value:.places$}", places = usize::from(places))
        };
        if matches!(self, Self::Auto) {
            label = label.trim_end_matches('0').trim_end_matches('.').to_owned();
        }
        if label.starts_with('-') && label.parse::<f64>() == Ok(0.0) {
            label.remove(0);
        }
        if percent {
            label.push('%');
        }
        label
    }
}
