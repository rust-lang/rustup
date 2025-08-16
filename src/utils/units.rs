use std::fmt::{self, Display};

/// Human readable size representation
#[derive(Copy, Clone, Debug)]
pub(crate) struct Size(usize);

impl Size {
    pub(crate) fn new(size: usize) -> Self {
        Self(size)
    }
}

impl Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const KI: f64 = 1024.0;
        const MI: f64 = KI * KI;
        const GI: f64 = KI * KI * KI;
        let size = self.0 as f64;

        let suffix: String = "".into();

        if size >= GI {
            write!(f, "{:5.1} GiB{}", size / GI, suffix)
        } else if size >= MI {
            write!(f, "{:5.1} MiB{}", size / MI, suffix)
        } else if size >= KI {
            write!(f, "{:5.1} KiB{}", size / KI, suffix)
        } else {
            write!(f, "{size:3.0} B{suffix}")
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unit_formatter_test() {
        use crate::utils::units::Size;

        // Test Bytes
        assert_eq!(format!("{}", Size::new(1)), "  1 B");
        assert_eq!(format!("{}", Size::new(1024)), "  1.0 KiB");
        assert_eq!(format!("{}", Size::new(1024usize.pow(2))), "  1.0 MiB");
        assert_eq!(format!("{}", Size::new(1024usize.pow(3))), "  1.0 GiB");
    }
}
