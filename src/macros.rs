
/// Construct a `condense_db::Value` from a JSON-like literal.
///
/// ```edition2018
/// # use condense_db::msgpack;
/// #
/// let value = msgpack!({
///     "title": "First Post",
///     "message": "This a test",
///     "public": true,
///     "index": 1,
///     "tags": [
///         "first",
///         "test",
///         "msgpack"
///     ]
/// });
/// ```
///
/// Variables or expressions can be interpolated into the literal. Any type
/// interpolated into an array element or object value must implement 
/// `Into<Value>`, while any type interpolated into an object key must implement 
/// `Into<String>`. If these conditions are not met, the `msgpack!` macro will 
/// panic.
///
/// ```edition2018
/// # use condense_db::msgpack;
/// # use std::time::SystemTime;
/// #
/// let title = "First Post";
/// let message = "This is a test";
/// let visibility = 3;
/// let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH);
/// let taglist = vec!["first", "test", "msgpack"];
///
/// let value = msgpack!({
///     "title": title,
///     "message": message,
///     "public": visibility > 0,
///     "time": Timestamp::from_sec(now.as_secs()),
///     "tags": taglist
/// });
/// ```
///
/// Trailing commas are allowed inside both arrays and objects.
///
/// ```edition2018
/// # use condense_db::msgpack;
/// #
/// let value = msgpack!([
///     "check",
///     "out",
///     "this",
///     "comma -->",
/// ]);
/// ```
#[macro_export(local_inner_macros)]
macro_rules! msgpack {
    // Hide distracting implementation details from the generated rustdoc.
    ($($msgpack:tt)+) => {
        msgpack_internal!($($msgpack)+)
    };
}

#[macro_export(local_inner_macros)]
#[doc(hidden)]
macro_rules! msgpack_internal {
    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an array [...]. Produces a vec![...]
    // of the elements.
    //
    // Must be invoked as: msgpack_internal!(@array [] $($tt)*)
    //////////////////////////////////////////////////////////////////////////

    // Done with trailing comma.
    (@array [$($elems:expr,)*]) => {
        msgpack_internal_vec![$($elems,)*]
    };

    // Done without trailing comma.
    (@array [$($elems:expr),*]) => {
        msgpack_internal_vec![$($elems),*]
    };

    // Next element is `null`.
    (@array [$($elems:expr,)*] null $($rest:tt)*) => {
        msgpack_internal!(@array [$($elems,)* msgpack_internal!(null)] $($rest)*)
    };

    // Next element is `true`.
    (@array [$($elems:expr,)*] true $($rest:tt)*) => {
        msgpack_internal!(@array [$($elems,)* msgpack_internal!(true)] $($rest)*)
    };

    // Next element is `false`.
    (@array [$($elems:expr,)*] false $($rest:tt)*) => {
        msgpack_internal!(@array [$($elems,)* msgpack_internal!(false)] $($rest)*)
    };

    // Next element is an array.
    (@array [$($elems:expr,)*] [$($array:tt)*] $($rest:tt)*) => {
        msgpack_internal!(@array [$($elems,)* msgpack_internal!([$($array)*])] $($rest)*)
    };

    // Next element is a map.
    (@array [$($elems:expr,)*] {$($map:tt)*} $($rest:tt)*) => {
        msgpack_internal!(@array [$($elems,)* msgpack_internal!({$($map)*})] $($rest)*)
    };

    // Next element is an expression followed by comma.
    (@array [$($elems:expr,)*] $next:expr, $($rest:tt)*) => {
        msgpack_internal!(@array [$($elems,)* msgpack_internal!($next),] $($rest)*)
    };

    // Last element is an expression with no trailing comma.
    (@array [$($elems:expr,)*] $last:expr) => {
        msgpack_internal!(@array [$($elems,)* msgpack_internal!($last)])
    };

    // Comma after the most recent element.
    (@array [$($elems:expr),*] , $($rest:tt)*) => {
        msgpack_internal!(@array [$($elems,)*] $($rest)*)
    };

    // Unexpected token after most recent element.
    (@array [$($elems:expr),*] $unexpected:tt $($rest:tt)*) => {
        msgpack_unexpected!($unexpected)
    };

    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an object {...}. Each entry is
    // inserted into the given map variable.
    //
    // Must be invoked as: msgpack_internal!(@object $map () ($($tt)*) ($($tt)*))
    //
    // We require two copies of the input tokens so that we can match on one
    // copy and trigger errors on the other copy.
    //////////////////////////////////////////////////////////////////////////

    // Done.
    (@object $object:ident () () ()) => {};

    // Insert the current entry followed by trailing comma.
    (@object $object:ident [$($key:tt)+] ($value:expr) , $($rest:tt)*) => {
        let _ = $object.insert(($($key)+).into(), $value);
        msgpack_internal!(@object $object () ($($rest)*) ($($rest)*));
    };

    // Current entry followed by unexpected token.
    (@object $object:ident [$($key:tt)+] ($value:expr) $unexpected:tt $($rest:tt)*) => {
        msgpack_unexpected!($unexpected);
    };

    // Insert the last entry without trailing comma.
    (@object $object:ident [$($key:tt)+] ($value:expr)) => {
        let _ = $object.insert(($($key)+).into(), $value);
    };

    // Next value is `null`.
    (@object $object:ident ($($key:tt)+) (: null $($rest:tt)*) $copy:tt) => {
        msgpack_internal!(@object $object [$($key)+] (msgpack_internal!(null)) $($rest)*);
    };

    // Next value is `true`.
    (@object $object:ident ($($key:tt)+) (: true $($rest:tt)*) $copy:tt) => {
        msgpack_internal!(@object $object [$($key)+] (msgpack_internal!(true)) $($rest)*);
    };

    // Next value is `false`.
    (@object $object:ident ($($key:tt)+) (: false $($rest:tt)*) $copy:tt) => {
        msgpack_internal!(@object $object [$($key)+] (msgpack_internal!(false)) $($rest)*);
    };

    // Next value is an array.
    (@object $object:ident ($($key:tt)+) (: [$($array:tt)*] $($rest:tt)*) $copy:tt) => {
        msgpack_internal!(@object $object [$($key)+] (msgpack_internal!([$($array)*])) $($rest)*);
    };

    // Next value is a map.
    (@object $object:ident ($($key:tt)+) (: {$($map:tt)*} $($rest:tt)*) $copy:tt) => {
        msgpack_internal!(@object $object [$($key)+] (msgpack_internal!({$($map)*})) $($rest)*);
    };

    // Next value is an expression followed by comma.
    (@object $object:ident ($($key:tt)+) (: $value:expr , $($rest:tt)*) $copy:tt) => {
        msgpack_internal!(@object $object [$($key)+] (msgpack_internal!($value)) , $($rest)*);
    };

    // Last value is an expression with no trailing comma.
    (@object $object:ident ($($key:tt)+) (: $value:expr) $copy:tt) => {
        msgpack_internal!(@object $object [$($key)+] (msgpack_internal!($value)));
    };

    // Missing value for last entry. Trigger a reasonable error message.
    (@object $object:ident ($($key:tt)+) (:) $copy:tt) => {
        // "unexpected end of macro invocation"
        msgpack_internal!();
    };

    // Missing colon and value for last entry. Trigger a reasonable error
    // message.
    (@object $object:ident ($($key:tt)+) () $copy:tt) => {
        // "unexpected end of macro invocation"
        msgpack_internal!();
    };

    // Misplaced colon. Trigger a reasonable error message.
    (@object $object:ident () (: $($rest:tt)*) ($colon:tt $($copy:tt)*)) => {
        // Takes no arguments so "no rules expected the token `:`".
        msgpack_unexpected!($colon);
    };

    // Found a comma inside a key. Trigger a reasonable error message.
    (@object $object:ident ($($key:tt)*) (, $($rest:tt)*) ($comma:tt $($copy:tt)*)) => {
        // Takes no arguments so "no rules expected the token `,`".
        msgpack_unexpected!($comma);
    };

    // Key is fully parenthesized. This avoids clippy double_parens false
    // positives because the parenthesization may be necessary here.
    (@object $object:ident () (($key:expr) : $($rest:tt)*) $copy:tt) => {
        msgpack_internal!(@object $object ($key) (: $($rest)*) (: $($rest)*));
    };

    // Munch a token into the current key.
    (@object $object:ident ($($key:tt)*) ($tt:tt $($rest:tt)*) $copy:tt) => {
        msgpack_internal!(@object $object ($($key)* $tt) ($($rest)*) ($($rest)*));
    };

    //////////////////////////////////////////////////////////////////////////
    // The main implementation.
    //
    // Must be invoked as: msgpack_internal!($($msgpack)+)
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
        $crate::Value::Array(msgpack_internal_vec![])
    };

    ([ $($tt:tt)+ ]) => {
        $crate::Value::Array(msgpack_internal!(@array [] $($tt)+))
    };

    ({}) => {
        $crate::Value::Object(std::collections::BTreeMap::new())
    };

    ({ $($tt:tt)+ }) => {
        $crate::Value::Object({
            let mut object = std::collections::BTreeMap::new();
            msgpack_internal!(@object object () ($($tt)+) ($($tt)+));
            object
        })
    };

    // Any Serialize type: numbers, strings, struct literals, variables etc.
    // Must be below every other rule.
    ($other:expr) => {
        $crate::Value::from($other)
    };
}

// The msgpack_internal macro above cannot invoke vec directly because it uses
// local_inner_macros. A vec invocation there would resolve to $crate::vec.
// Instead invoke vec here outside of local_inner_macros.
#[macro_export]
#[doc(hidden)]
macro_rules! msgpack_internal_vec {
    ($($content:tt)*) => {
        vec![$($content)*]
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! msgpack_unexpected {
    () => {};
}
