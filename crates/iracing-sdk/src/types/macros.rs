macro_rules! ensure {
    ($condition:expr, $error:expr $(,)?) => {
        if !$condition {
            return Err($error);
        }
    };
}

macro_rules! ensure_cmp {
    ($value:expr, $field:ident, $op:tt, $bound:expr, $context:expr, $message:literal) => {{
        let value = &$value.$field;
        let bound = &$bound;

        ensure!(
            value $op bound,
            $crate::IRacingSDKError::parse_error(
                $context,
                format!(
                    concat!(stringify!($field), " ", $message, " {}"),
                    bound,
                ),
            ),
        );
    }};
}

macro_rules! ensure_eq {
    ($value:expr, $field:ident, $bound:expr, $context:expr $(,)?) => {
        $crate::types::macros::ensure_cmp!(
            $value,
            $field,
            ==,
            $bound,
            $context,
            "must be equal to"
        )
    };
}

macro_rules! ensure_gt {
    ($value:expr, $field:ident, $bound:expr, $context:expr $(,)?) => {
        $crate::types::macros::ensure_cmp!(
            $value,
            $field,
            >,
            $bound,
            $context,
            "must be greater than"
        )
    };
}

macro_rules! ensure_gte {
    ($value:expr, $field:ident, $bound:expr, $context:expr $(,)?) => {
        $crate::types::macros::ensure_cmp!(
            $value,
            $field,
            >=,
            $bound,
            $context,
            "must be greater than or equal to"
        )
    };
}

macro_rules! ensure_positive {
    ($value:expr, $field:ident, $context:expr) => {
        $crate::types::macros::ensure_gt!($value, $field, 0, $context)
    };
}

macro_rules! ensure_nonnegative {
    ($value:expr, $field:ident, $context:expr $(,)?) => {
        $crate::types::macros::ensure_gte!($value, $field, 0, $context)
    };
}

pub(crate) use ensure_cmp;
pub(crate) use ensure_gt;
pub(crate) use ensure_gte;
