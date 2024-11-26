#[allow(unused_imports)]
use crate::TypeConfig;

/// Define types for a SuRaft type configuration.
///
/// Since Rust has some limitations when deriving traits for types with generic
/// arguments and most types are parameterized by [`TypeConfig`], we need to
/// add supertraits to a type implementing [`TypeConfig`].
///
/// This macro does exactly that.
///
/// Example:
/// ```ignore
/// suraft::declare_suraft_types!(
///    pub TypeConfig:
///        AppData      = ClientRequest,
///        Responder    = suraft::impls::OneshotResponder<TypeConfig>,
///        AsyncRuntime = suraft::TokioRuntime,
/// );
/// ```
///
/// Types can be omitted, and the following default type will be used:
/// - `AppData`:      `String`
/// - `Responder`:    `::suraft::impls::OneshotResponder<Self>`
/// - `AsyncRuntime`: `::suraft::impls::TokioRuntime`
///
/// For example, to declare with only `D` and `R` types:
/// ```ignore
/// suraft::declare_suraft_types!(
///    pub TypeConfig:
///        AppData     = ClientRequest,
/// );
/// ```
///
/// Or just use the default type config:
/// ```ignore
/// suraft::declare_suraft_types!(pub TypeConfig);
/// ```
#[macro_export]
macro_rules! declare_suraft_types {
    // Add a trailing colon to    `declare_suraft_types(MyType)`,
    // Make it the standard form: `declare_suraft_types(MyType:)`.
    ($(#[$outer:meta])* $visibility:vis $id:ident) => {
        $crate::declare_suraft_types!($(#[$outer])* $visibility $id:);
    };

    // The main entry of this macro
    ($(#[$outer:meta])* $visibility:vis $id:ident: $($(#[$inner:meta])* $type_id:ident = $type:ty),* $(,)? ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd)]
        #[derive(serde::Deserialize, serde::Serialize)]
        $visibility struct $id {}

        impl $crate::TypeConfig for $id {
            // `expand!(KEYED, ...)` ignores the duplicates.
            // Thus by appending default types after user defined types,
            // the absent user defined types are filled with default types.
            $crate::openraft_macros::expand!(
                KEYED,
                (T, ATTR, V) => {ATTR type T = V;},
                $(($type_id, $(#[$inner])*, $type),)*

                // Default types:
                (AppData      , , String                                ),
                (Responder    , , $crate::impls::OneshotResponder<Self> ),
                (AsyncRuntime , , $crate::impls::TokioRuntime           ),
            );

        }
    };
}
