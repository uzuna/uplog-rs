use criterion::{criterion_group, criterion_main, Criterion};
use fake::{Dummy, Fake, Faker};
use uplog::{devlog, devlog_encode, session_init};

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
        let mut buf = [0_u8; 1024];
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
                serde_cbor::to_writer(&mut buf[..], &r).unwrap();
                assert!(!buf.is_empty());
            }
        })
    });

    c.bench_function("generate log borrow short data 1000", |b| {
        let mut buf = [0_u8; 1024];
        b.iter(|| {
            for v in &testdata {
                devlog_encode!(
                    &mut buf[..],
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
                assert!(!buf.is_empty());
            }
        })
    });

    c.bench_function("generate log 500KB data", |b| {
        let mut buf = vec![0_u8; 1024 * 1024 * 501];
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
            serde_cbor::to_writer(&mut buf, &r).unwrap();
            assert!(!buf.is_empty());
        })
    });

    c.bench_function("generate log borrow 500KB data", |b| {
        let mut buf = vec![0_u8; 1024 * 1024 * 501];
        b.iter(|| {
            devlog_encode!(
                &mut buf,
                uplog::Level::Info,
                "uplpg::benches",
                "large data",
                "type",
                "dummy/bynary",
                "data",
                &largedata[..]
            );
            assert!(!buf.is_empty());
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
