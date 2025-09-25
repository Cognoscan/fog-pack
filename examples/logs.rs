use fog_pack::{document::*, schema::NoSchema};
use rand::Rng;
use std::error::Error;
use std::mem;
use std::ops;

pub trait Generate {
    fn generate<R: Rng>(rng: &mut R) -> Self;
}

impl Generate for () {
    fn generate<R: Rng>(_: &mut R) -> Self {}
}

impl Generate for bool {
    fn generate<R: Rng>(rng: &mut R) -> Self {
        rng.gen_bool(0.5)
    }
}

macro_rules! impl_generate {
    ($ty:ty) => {
        impl Generate for $ty {
            fn generate<R: Rng>(rng: &mut R) -> Self {
                rng.r#gen()
            }
        }
    };
}

impl_generate!(u8);
impl_generate!(u16);
impl_generate!(u32);
impl_generate!(u64);
impl_generate!(u128);
impl_generate!(usize);
impl_generate!(i8);
impl_generate!(i16);
impl_generate!(i32);
impl_generate!(i64);
impl_generate!(i128);
impl_generate!(isize);
impl_generate!(f32);
impl_generate!(f64);

macro_rules! impl_tuple {
    () => {};
    ($first:ident, $($rest:ident,)*) => {
        impl<$first: Generate, $($rest: Generate,)*> Generate for ($first, $($rest,)*) {
            fn generate<R: Rng>(rng: &mut R) -> Self {
                ($first::generate(rng), $($rest::generate(rng),)*)
            }
        }

        impl_tuple!($($rest,)*);
    };
}

impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11,);

macro_rules! impl_array {
    () => {};
    ($len:literal, $($rest:literal,)*) => {
        impl<T: Generate> Generate for [T; $len] {
            fn generate<R: Rng>(rng: &mut R) -> Self {
                let mut result = mem::MaybeUninit::<Self>::uninit();
                let result_ptr = result.as_mut_ptr().cast::<T>();
                #[allow(clippy::reversed_empty_ranges)]
                for i in 0..$len {
                    unsafe {
                        result_ptr.add(i).write(T::generate(rng));
                    }
                }
                unsafe {
                    result.assume_init()
                }
            }
        }

        impl_array!($($rest,)*);
    }
}

impl_array!(
    31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8,
    7, 6, 5, 4, 3, 2, 1, 0,
);

impl<T: Generate> Generate for Option<T> {
    fn generate<R: Rng>(rng: &mut R) -> Self {
        if rng.gen_bool(0.5) {
            Some(T::generate(rng))
        } else {
            None
        }
    }
}

pub fn generate_vec<R: Rng, T: Generate>(rng: &mut R, range: ops::Range<usize>) -> Vec<T> {
    let len = rng.gen_range(range.start..range.end);
    let mut result = Vec::with_capacity(len);
    for _ in 0..len {
        result.push(T::generate(rng));
    }
    result
}

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Address {
    pub x0: u8,
    pub x1: u8,
    pub x2: u8,
    pub x3: u8,
}

impl Generate for Address {
    fn generate<R: Rng>(rand: &mut R) -> Self {
        Self {
            x0: rand.r#gen(),
            x1: rand.r#gen(),
            x2: rand.r#gen(),
            x3: rand.r#gen(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Log {
    pub address: Address,
    pub code: u16,
    pub date: String,
    pub identity: String,
    pub request: String,
    pub size: u64,
    pub userid: String,
}

impl Generate for Log {
    fn generate<R: Rng>(rand: &mut R) -> Self {
        const USERID: [&str; 9] = [
            "-", "alice", "bob", "carmen", "david", "eric", "frank", "george", "harry",
        ];
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        const TIMEZONE: [&str; 25] = [
            "-1200", "-1100", "-1000", "-0900", "-0800", "-0700", "-0600", "-0500", "-0400",
            "-0300", "-0200", "-0100", "+0000", "+0100", "+0200", "+0300", "+0400", "+0500",
            "+0600", "+0700", "+0800", "+0900", "+1000", "+1100", "+1200",
        ];
        let date = format!(
            "{}/{}/{}:{}:{}:{} {}",
            rand.gen_range(1..29),
            MONTHS[rand.gen_range(0..12)],
            rand.gen_range(1970..2022),
            rand.gen_range(0..24),
            rand.gen_range(0..60),
            rand.gen_range(0..60),
            TIMEZONE[rand.gen_range(0..25)],
        );
        const CODES: [u16; 63] = [
            100, 101, 102, 103, 200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300, 301, 302,
            303, 304, 305, 306, 307, 308, 400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410,
            411, 412, 413, 414, 415, 416, 417, 418, 421, 422, 423, 424, 425, 426, 428, 429, 431,
            451, 500, 501, 502, 503, 504, 505, 506, 507, 508, 510, 511,
        ];
        const METHODS: [&str; 5] = ["GET", "POST", "PUT", "UPDATE", "DELETE"];
        const ROUTES: [&str; 7] = [
            "/favicon.ico",
            "/css/index.css",
            "/css/font-awsome.min.css",
            "/img/logo-full.svg",
            "/img/splash.jpg",
            "/api/login",
            "/api/logout",
        ];
        const PROTOCOLS: [&str; 4] = ["HTTP/1.0", "HTTP/1.1", "HTTP/2", "HTTP/3"];
        let request = format!(
            "{} {} {}",
            METHODS[rand.gen_range(0..5)],
            ROUTES[rand.gen_range(0..7)],
            PROTOCOLS[rand.gen_range(0..4)],
        );
        Self {
            address: Address::generate(rand),
            code: CODES[rand.gen_range(0..CODES.len())],
            date,
            identity: "-".into(),
            request,
            size: rand.gen_range(0..100_000_000),
            userid: USERID[rand.gen_range(0..USERID.len())].into(),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Generate a whole pile of log items
    let mut rng = rand::thread_rng();
    const LOGS: usize = 10_000;
    let logs = generate_vec::<_, Log>(&mut rng, LOGS..LOGS + 1);
    // Try to make them into documents
    let builder = VecDocumentBuilder::new_ordered(logs.iter(), None);
    let mut docs = builder.collect::<Result<Vec<NewDocument>, fog_pack::error::Error>>()?;

    let docs: Vec<Document> = docs
        .drain(0..)
        .map(NoSchema::validate_new_doc)
        .collect::<Result<Vec<Document>, fog_pack::error::Error>>()?;

    for i in 0..1000 {
        let dec_logs: Vec<Log> = docs
            .iter()
            .flat_map(|doc| doc.deserialize::<Vec<Log>>().unwrap())
            .collect();
        println!("#{}: Decoded {} documents overall", i, dec_logs.len());
        //let sum: usize = docs.iter().map(|doc| {
        //    let parser = fog_pack::element::Parser::new(doc.data());
        //    parser.count()
        //}).sum();
        //println!("#{}: Decoded {} elements overall", i, sum);
        //assert!(dec_logs == logs, "Didn't decode identically");
    }
    Ok(())
}
