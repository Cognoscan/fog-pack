Regular Expressions
===================

Regular expression (regex) syntaxes vary across programming languages and 
libraries. Condense-DB uses similar syntax to XML Schema regexes,
and likewise can only be used for determining if a string matches or does not 
match the given expression.

All regexes are implicitly anchored, unlike some other languages, as the most 
common use is to match entire literals. So, a regex like `[0-9]+` would be 
written like `^[0-9]+$` in other languages.

Although specific implementations may support more than the syntax specified 
here, only the syntax specified here is considered universal across all 
implementations.

Unicode Support
---------------

A regular expression implementation should conform to the "Unicode Technical 
Standard #18 - Unicode Regular Expressions" specification, version 19. It must 
only meet the requirements for
[Basic Unicode Support](http://unicode.org/reports/tr18/#Basic_Unicode_Support), 
with the following exceptions:

1. Line boundaries are assumed to not be Unicode-aware. Only `\n` is recognized 
	as a line boundary.
2. Compatibility properties specified by 
	[RL1.2a](http://unicode.org/reports/tr18/#RL1.2a) are ASCII-only definitions.

Metacharacters
--------------
Metacharacters have special meanings in regular expressions, and must be escaped 
with the `\\` operator to signify themselves. All other characters match 
themselves by default.

```
.    Any UTF-8 character except `\n`
\    Metacharacter escape
?    Zero or one occurances
*    Zero or more occurances
+    One or more occurances
|    The "Or" operator
{}   Occurance range operator
()   Grouping operator
[]   Character class expression operator
^    Reserved - marks the start of the regex in some parsers
$    Reserved - marks the end of the regex in some parsers
```

Composites
----------

Composite regex sequences are constructed by concatenating them or by using the 
`|` operator to allow one or the other, preferring the first:

- `xy` requires `x` followed by `y`
- `x|y` allows `x` or `y`, preferring `x`

Character Classes
-----------------
Character classes may be defined by the `[]` operators, supporting ranges, 
intersection, subtraction, and symmetric difference operations. All Unicode 
character classes defined in [UTS#18](http://unicode.org/reports/tr18/) may be 
used. 

Within a character class definition, the following characters have special 
meanings (in order of precedence):

- `^` at the start will cause matching against any character not in the class
- `[x]` defines a nested character class containing `x`
- `-` defines a range between the preceeding and subsequent classes
- `&&` defines intersection between classes
- `--` defines subtraction of the subsequent class from the preceeding one
- `~~` defines symmetric difference between classes

Besides the Unicode character classes, several additional character classes are 
predefined:

- `.` is any character except the newline
- `\d` is the digit class (`\p{Nd}`)
- `\D` is the negation of `\d`
- `\s` is whitespace (`\p{White_Space}`)
- `\S` is the negation of `\s`
- `\w` is a word character (`[\p{Alphabetic}\p{M}\d\p{Pc}\p{Join_Control}]`)
- `\W` is the negation of `\w`

Escape Sequences
----------------
The following escape sequences are supported:

- Any of the metacharacters may be escaped - `\\.+*?()|[]{}^$`
- `\a` denotes the bell character (`\x07`)
- `\f` denotes form feed (`\x0C`)
- `\t` denotes horizontal tab
- `\n` denotes new line
- `\r` denotes carriage return
- `\v` denotes vertical tab (`\x0B`)
- `\x{10FFFF}` allows any hex character code for a code point
- `\u{10FFFF}` allows any hex character code for a code point
- `\U{10FFFF}` allows any hex character code for a code point
- `\x7F` allows any hex code (exactly 2 digits)
- `\u007F` allows any hex code (exactly 4 digits)
- `\U0000007F` allows any hex code (exactly 8 digits)


Repetitions
-----------
The following greedy repetition syntaxes are supported:

- `x*` matches zero or more of `x`
- `x+` matches one or more of `x`
- `x?` matches zero or one `x`
- `x{n,m}` matches at least n `x` and at most m `x`
- `x{n,}` matches at least n `x`
- `x{n}` matches exactly n `x`

Ungreedy repetitions are not supported.

Grouping
--------
Expressions may be grouped with parentheses. The expression must not start with 
a `?`.














