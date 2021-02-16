
/// Construct a `fog_pack::Value` from a JSON-like literal.
///
/// ```
/// # use fog_pack::fogpack;
/// #
/// let value = fogpack!({
///     "title": "First Post",
///     "message": "This a test",
///     "public": true,
///     "index": 1,
///     "tags": [
///         "first",
///         "test",
///         "fogpack"
///     ]
/// });
/// ```
///
/// Variables or expressions can be interpolated into the literal. Any type 
/// interpolated into an array element or object value must implement 
/// `Into<Value>`, while any type interpolated into an object key must implement 
/// `Into<String>`. If these conditions are not met, the `fogpack!` macro will 
/// panic.
///
/// Importantly, a Vec or a slice must be of type `Value` to work, as the parser 
/// only accepts Vec/slices of type u8 (for the fogpack Binary type) or of type 
/// Value.
///
/// ```
/// # use fog_pack::{fogpack, Value, Timestamp};
/// # use std::time::SystemTime;
/// #
/// let title = "First Post";
/// let message = "This is a test";
/// let visibility = 3;
/// let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
/// let taglist: Vec<Value> = 
///     vec!["first", "test", "fogpack"]
///     .iter()
///     .map(|x| Value::from(*x))
///     .collect();
///
/// let value = fogpack!({
///     "title": title,
///     "message": message,
///     "public": visibility > 0,
///     "time": Timestamp::from_sec(now.as_secs() as i64),
///     "tags": taglist
/// });
/// ```
///
/// Trailing commas are allowed inside both arrays and objects.
///
/// ```
/// # use fog_pack::fogpack;
/// #
/// let value = fogpack!([
///     "check",
///     "out",
///     "this",
///     "comma -->",
/// ]);
/// ```
#[macro_export(local_inner_macros)]
macro_rules! fogpack {
    // Hide distracting implementation details from the generated rustdoc.
    ($($fogpack:tt)+) => {
        fogpack_internal!($($fogpack)+)
    };
}

#[macro_export(local_inner_macros)]
#[doc(hidden)]
macro_rules! fogpack_internal {
    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an array [...]. Produces a vec![...]
    // of the elements.
    //
    // Must be invoked as: fogpack_internal!(@array [] $($tt)*)
    //////////////////////////////////////////////////////////////////////////

    // Done with trailing comma.
    (@array [$($elems:expr,)*]) => {
        fogpack_internal_vec![$($elems,)*]
    };

    // Done without trailing comma.
    (@array [$($elems:expr),*]) => {
        fogpack_internal_vec![$($elems),*]
    };

    // Next element is `null`.
    (@array [$($elems:expr,)*] null $($rest:tt)*) => {
        fogpack_internal!(@array [$($elems,)* fogpack_internal!(null)] $($rest)*)
    };

    // Next element is `true`.
    (@array [$($elems:expr,)*] true $($rest:tt)*) => {
        fogpack_internal!(@array [$($elems,)* fogpack_internal!(true)] $($rest)*)
    };

    // Next element is `false`.
    (@array [$($elems:expr,)*] false $($rest:tt)*) => {
        fogpack_internal!(@array [$($elems,)* fogpack_internal!(false)] $($rest)*)
    };

    // Next element is an array.
    (@array [$($elems:expr,)*] [$($array:tt)*] $($rest:tt)*) => {
        fogpack_internal!(@array [$($elems,)* fogpack_internal!([$($array)*])] $($rest)*)
    };

    // Next element is a map.
    (@array [$($elems:expr,)*] {$($map:tt)*} $($rest:tt)*) => {
        fogpack_internal!(@array [$($elems,)* fogpack_internal!({$($map)*})] $($rest)*)
    };

    // Next element is an expression followed by comma.
    (@array [$($elems:expr,)*] $next:expr, $($rest:tt)*) => {
        fogpack_internal!(@array [$($elems,)* fogpack_internal!($next),] $($rest)*)
    };

    // Last element is an expression with no trailing comma.
    (@array [$($elems:expr,)*] $last:expr) => {
        fogpack_internal!(@array [$($elems,)* fogpack_internal!($last)])
    };

    // Comma after the most recent element.
    (@array [$($elems:expr),*] , $($rest:tt)*) => {
        fogpack_internal!(@array [$($elems,)*] $($rest)*)
    };

    // Unexpected token after most recent element.
    (@array [$($elems:expr),*] $unexpected:tt $($rest:tt)*) => {
        fogpack_unexpected!($unexpected)
    };

    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an object {...}. Each entry is
    // inserted into the given map variable.
    //
    // Must be invoked as: fogpack_internal!(@object $map () ($($tt)*) ($($tt)*))
    //
    // We require two copies of the input tokens so that we can match on one
    // copy and trigger errors on the other copy.
    //////////////////////////////////////////////////////////////////////////

    // Done.
    (@object $object:ident () () ()) => {};

    // Insert the current entry followed by trailing comma.
    (@object $object:ident [$($key:tt)+] ($value:expr) , $($rest:tt)*) => {
        let _ = $object.insert(($($key)+).into(), $value);
        fogpack_internal!(@object $object () ($($rest)*) ($($rest)*));
    };

    // Current entry followed by unexpected token.
    (@object $object:ident [$($key:tt)+] ($value:expr) $unexpected:tt $($rest:tt)*) => {
        fogpack_unexpected!($unexpected);
    };

    // Insert the last entry without trailing comma.
    (@object $object:ident [$($key:tt)+] ($value:expr)) => {
        let _ = $object.insert(($($key)+).into(), $value);
    };

    // Next value is `null`.
    (@object $object:ident ($($key:tt)+) (: null $($rest:tt)*) $copy:tt) => {
        fogpack_internal!(@object $object [$($key)+] (fogpack_internal!(null)) $($rest)*);
    };

    // Next value is `true`.
    (@object $object:ident ($($key:tt)+) (: true $($rest:tt)*) $copy:tt) => {
        fogpack_internal!(@object $object [$($key)+] (fogpack_internal!(true)) $($rest)*);
    };

    // Next value is `false`.
    (@object $object:ident ($($key:tt)+) (: false $($rest:tt)*) $copy:tt) => {
        fogpack_internal!(@object $object [$($key)+] (fogpack_internal!(false)) $($rest)*);
    };

    // Next value is an array.
    (@object $object:ident ($($key:tt)+) (: [$($array:tt)*] $($rest:tt)*) $copy:tt) => {
        fogpack_internal!(@object $object [$($key)+] (fogpack_internal!([$($array)*])) $($rest)*);
    };

    // Next value is a map.
    (@object $object:ident ($($key:tt)+) (: {$($map:tt)*} $($rest:tt)*) $copy:tt) => {
        fogpack_internal!(@object $object [$($key)+] (fogpack_internal!({$($map)*})) $($rest)*);
    };

    // Next value is an expression followed by comma.
    (@object $object:ident ($($key:tt)+) (: $value:expr , $($rest:tt)*) $copy:tt) => {
        fogpack_internal!(@object $object [$($key)+] (fogpack_internal!($value)) , $($rest)*);
    };

    // Last value is an expression with no trailing comma.
    (@object $object:ident ($($key:tt)+) (: $value:expr) $copy:tt) => {
        fogpack_internal!(@object $object [$($key)+] (fogpack_internal!($value)));
    };

    // Missing value for last entry. Trigger a reasonable error message.
    (@object $object:ident ($($key:tt)+) (:) $copy:tt) => {
        // "unexpected end of macro invocation"
        fogpack_internal!();
    };

    // Missing colon and value for last entry. Trigger a reasonable error
    // message.
    (@object $object:ident ($($key:tt)+) () $copy:tt) => {
        // "unexpected end of macro invocation"
        fogpack_internal!();
    };

    // Misplaced colon. Trigger a reasonable error message.
    (@object $object:ident () (: $($rest:tt)*) ($colon:tt $($copy:tt)*)) => {
        // Takes no arguments so "no rules expected the token `:`".
        fogpack_unexpected!($colon);
    };

    // Found a comma inside a key. Trigger a reasonable error message.
    (@object $object:ident ($($key:tt)*) (, $($rest:tt)*) ($comma:tt $($copy:tt)*)) => {
        // Takes no arguments so "no rules expected the token `,`".
        fogpack_unexpected!($comma);
    };

    // Key is fully parenthesized. This avoids clippy double_parens false
    // positives because the parenthesization may be necessary here.
    (@object $object:ident () (($key:expr) : $($rest:tt)*) $copy:tt) => {
        fogpack_internal!(@object $object ($key) (: $($rest)*) (: $($rest)*));
    };

    // Munch a token into the current key.
    (@object $object:ident ($($key:tt)*) ($tt:tt $($rest:tt)*) $copy:tt) => {
        fogpack_internal!(@object $object ($($key)* $tt) ($($rest)*) ($($rest)*));
    };

    //////////////////////////////////////////////////////////////////////////
    // The main implementation.
    //
    // Must be invoked as: fogpack_internal!($($fogpack)+)
    //////////////////////////////////////////////////////////////////////////

    (null) => {
        $crate::Value::Null
    };

    (true) => {
        $crate::Value::Boolean(true)
    };

    (false) => {
        $crate::Value::Boolean(false)
    };

    ([]) => {
        $crate::Value::Array(fogpack_internal_vec![])
    };

    ([ $($tt:tt)+ ]) => {
        $crate::Value::Array(fogpack_internal!(@array [] $($tt)+))
    };

    ({}) => {
        $crate::Value::Object(std::collections::BTreeMap::new())
    };

    ({ $($tt:tt)+ }) => {
        $crate::Value::Object({
            let mut object = std::collections::BTreeMap::new();
            fogpack_internal!(@object object () ($($tt)+) ($($tt)+));
            object
        })
    };

    // Any Serialize type: numbers, strings, struct literals, variables etc.
    // Must be below every other rule.
    ($other:expr) => {
        $crate::Value::from($other)
    };
}

// The fogpack_internal macro above cannot invoke vec directly because it uses
// local_inner_macros. A vec invocation there would resolve to $crate::vec.
// Instead invoke vec here outside of local_inner_macros.
#[macro_export]
#[doc(hidden)]
macro_rules! fogpack_internal_vec {
    ($($content:tt)*) => {
        vec![$($content)*]
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! fogpack_unexpected {
    () => {};
}
