use criterion::{criterion_group, criterion_main, Criterion};
use fake::{Dummy, Fake, Faker};
use uplog::{devlog, session_init};

#[derive(Debug, Dummy)]
pub struct DummeData {
    #[dummy(faker = "100_000..2_000_000_000_000_000")]
    order_id: u64,
    customer: String,
    paid: bool,
}

fn criterion_benchmark(c: &mut Criterion) {
    session_init();
    let testdata: Vec<DummeData> = (0..1000).into_iter().map(|_| Faker.fake()).collect();
    let largedata = vec![64_u8; 1024 * 1024 * 500];

    c.bench_function("generate log short data 1000", |b| {
        b.iter(|| {
            for v in &testdata {
                let r = devlog!(
                    uplog::Level::Info,
                    "uplpg::benches",
                    "short log",
                    "order_id",
                    v.order_id,
                    "customer",
                    v.customer.as_str(),
                    "paid",
                    v.paid
                );
                let buf = serde_cbor::to_vec(&r).unwrap();
                assert!(!buf.is_empty());
            }
        })
    });

    c.bench_function("generate log 500KB data", |b| {
        b.iter(|| {
            let r = devlog!(
                uplog::Level::Info,
                "uplpg::benches",
                "large data",
                "type",
                "dummy/bynary",
                "data",
                &largedata[..]
            );
            let buf = serde_cbor::to_vec(&r).unwrap();
            assert!(!buf.is_empty());
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
