//! Various benchmarks for atomic_lib.
//! Should be run using `cargo criterion` or `cargo bench --all-features`.
//! See contribute.md for more information.

use atomic_lib::utils::random_string;
use atomic_lib::*;
use atomic_lib::resources::PropVals;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[cfg(feature = "db")]
use atomic_lib::similarity::SimilarityAlgorithm;

fn random_atom_string() -> Atom {
    Atom::new(
        format!("https://localhost/{}", random_string(10)),
        urls::DESCRIPTION.into(),
        Value::Markdown(random_string(200)),
    )
}

fn random_subject() -> String {
    format!("https://localhost/{}", random_string(10))
}

fn random_array(n: usize) -> Vec<String> {
    (0..n).map(|_| random_subject()).collect()
}

fn random_atom_array() -> Atom {
    Atom::new(
        format!("https://localhost/{}", random_string(10)),
        urls::COLLECTION_MEMBERS.into(),
        random_array(200).into(),
    )
}

fn random_resource(atom: &Atom) -> Resource {
    let mut resource = Resource::new(atom.subject.clone());
    resource.set_unsafe(atom.property.clone(), atom.value.clone());
    resource
}

fn criterion_benchmark(c: &mut Criterion) {
    let store = Db::init_temp("bench").unwrap();

    c.bench_function("add_resource", |b| {
        b.iter(|| {
            let resource = random_resource(&random_atom_string());
            store
                .add_resource_opts(&resource, true, true, false)
                .unwrap();
        })
    });

    c.bench_function("resource.save() string", |b| {
        b.iter(|| {
            let mut resource = random_resource(&random_atom_string());
            resource.save(&store).unwrap();
        })
    });

    c.bench_function("resource.save() array", |b| {
        b.iter(|| {
            let mut resource = random_resource(&random_atom_array());
            resource.save(&store).unwrap();
        })
    });

    let big_resource = store
        .get_resource_extended(
            "https://localhost/collections",
            false,
            &agents::ForAgent::Public,
        )
        .unwrap();

    c.bench_function("resource.to_json_ad()", |b| {
        b.iter(|| {
            big_resource.to_json_ad().unwrap();
        })
    });

    c.bench_function("resource.to_json_ld()", |b| {
        b.iter(|| {
            big_resource.to_json_ld(&store).unwrap();
        })
    });

    c.bench_function("resource.to_json()", |b| {
        b.iter(|| {
            big_resource.to_json(&store).unwrap();
        })
    });

    c.bench_function("resource.to_n_triples()", |b| {
        b.iter(|| {
            big_resource.to_n_triples(&store).unwrap();
        })
    });

    c.bench_function("all_resources()", |b| {
        b.iter(|| {
            let _all = store.all_resources(false).collect::<Vec<Resource>>();
        })
    });

    store.clear_all_danger().unwrap();
}

#[cfg(feature = "db")]
fn search_benchmarks(c: &mut Criterion) {
    use atomic_lib::search_sqlite::SqliteSearchState;

    let store = Db::init_temp("search_bench").unwrap();

    // Populate the store with test data for search benchmarks
    let test_resources = [
        (
            "https://localhost/atomic-data-model",
            "Atomic Data Model",
            "A semantic data model for graph data",
        ),
        (
            "https://localhost/atomic-server",
            "Atomic Server",
            "A fast and secure graph database",
        ),
        (
            "https://localhost/json-ld",
            "JSON-LD",
            "A JSON-based serialization for Linked Data",
        ),
        (
            "https://localhost/rdf-turtle",
            "RDF Turtle",
            "A human-readable RDF serialization",
        ),
        (
            "https://localhost/semantic-web",
            "Semantic Web",
            "A web of linked data using standards",
        ),
        (
            "https://localhost/knowledge-graph",
            "Knowledge Graph",
            "A graph-based representation of knowledge",
        ),
        (
            "https://localhost/triple-store",
            "Triple Store",
            "A database for storing RDF triples",
        ),
        (
            "https://localhost/sparql-query",
            "SPARQL Query",
            "A query language for RDF data",
        ),
        (
            "https://localhost/data-model",
            "Data Model",
            "A structure for organizing data",
        ),
        (
            "https://localhost/graph-database",
            "Graph Database",
            "A database using graph structures",
        ),
    ];

    for (subject, title, description) in test_resources {
        let mut resource = Resource::new(subject.to_string());
        resource.set_unsafe(urls::NAME.into(), Value::String(title.to_string()));
        resource.set_unsafe(
            urls::DESCRIPTION.into(),
            Value::String(description.to_string()),
        );
        store
            .add_resource_opts(&resource, true, true, false)
            .unwrap();
    }

    // Initialize search state
    let search_state = SqliteSearchState::new(store.clone()).unwrap();
    search_state.add_all_resources(&store).unwrap(); // This is the lib API, not server

    // Benchmark traditional text search
    c.bench_function("search/text_search", |b| {
        b.iter(|| search_state.text_search("atomic", 10).unwrap())
    });

    // Benchmark fuzzy search
    c.bench_function("search/fuzzy_search", |b| {
        b.iter(|| search_state.fuzzy_search("atomik", 2, 10).unwrap())
    });

    // Benchmark new similarity-based search
    c.bench_function("search/similarity_search_jaro_winkler", |b| {
        b.iter(|| {
            search_state
                .similarity_search("atomic", 10, SimilarityAlgorithm::JaroWinkler)
                .unwrap()
        })
    });

    c.bench_function("search/similarity_search_levenshtein", |b| {
        b.iter(|| {
            search_state
                .similarity_search("atomic", 10, SimilarityAlgorithm::Levenshtein)
                .unwrap()
        })
    });

    // Benchmark fuzzy similarity search
    c.bench_function("search/fuzzy_similarity_search", |b| {
        b.iter(|| {
            search_state
                .fuzzy_similarity_search("atomik", 2, 10, SimilarityAlgorithm::JaroWinkler)
                .unwrap()
        })
    });

    // Benchmark hierarchy search
    c.bench_function("search/hierarchy_search", |b| {
        b.iter(|| search_state.hierarchy_search("localhost", 10).unwrap())
    });

    // Benchmark terraphim fuzzy search if feature is enabled
    // NOTE: terraphim-search feature is not defined in Cargo.toml, removing
    // #[cfg(feature = "terraphim-search")]
    // c.bench_function("search/terraphim_fuzzy_search", |b| {
    //     b.iter(|| {
    //         search_state
    //             .terraphim_fuzzy_search("atomic", 0.6, 10)
    //             .unwrap()
    //     })
    // });

    // Benchmark cache performance by running searches again (should hit cache)
    c.bench_function("search/text_search_cached", |b| {
        // Prime the cache first
        let _ = search_state.text_search("atomic", 10);
        b.iter(|| search_state.text_search("atomic", 10).unwrap())
    });

    c.bench_function("search/fuzzy_search_cached", |b| {
        // Prime the cache first
        let _ = search_state.fuzzy_search("atomic", 2, 10);
        b.iter(|| search_state.fuzzy_search("atomic", 2, 10).unwrap())
    });

    // Benchmark memory-mapped FST access performance
    c.bench_function("search/fst_memory_mapped_access", |b| {
        b.iter(|| search_state.get_or_load_fst().unwrap())
    });

    // Benchmark different similarity algorithms head-to-head
    c.bench_function("search/similarity_jaro_vs_levenshtein", |b| {
        b.iter(|| {
            let _jaro_results = search_state
                .similarity_search("atomic", 10, SimilarityAlgorithm::JaroWinkler)
                .unwrap();
            let _levenshtein_results = search_state
                .similarity_search("atomic", 10, SimilarityAlgorithm::Levenshtein)
                .unwrap();
        })
    });

    // Benchmark add_resource to search index performance
    c.bench_function("search/add_resource", |b| {
        b.iter(|| {
            let resource = random_resource(&random_atom_string());
            let conn = store.get_connection().unwrap();
            search_state.add_resource(&resource, &conn).unwrap();
        })
    });

    // Benchmark add_resource with hierarchy caching
    c.bench_function("search/add_resource_with_hierarchy", |b| {
        b.iter(|| {
            let mut resource = random_resource(&random_atom_string());
            // Add a parent relationship to test hierarchy caching
            resource.set_unsafe(
                urls::PARENT.into(),
                Value::AtomicUrl("https://localhost/parent".into()),
            );
            let conn = store.get_connection().unwrap();
            search_state.add_resource(&resource, &conn).unwrap();
        })
    });

    // Benchmark concurrent add_resource operations
    c.bench_function("search/add_resource_concurrent", |b| {
        use std::sync::Arc;
        use std::thread;

        b.iter(|| {
            let search_state = Arc::new(search_state.clone());
            let store = Arc::new(store.clone());

            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let search_state = search_state.clone();
                    let store = store.clone();
                    thread::spawn(move || {
                        let resource = random_resource(&random_atom_string());
                        let conn = store.get_connection().unwrap();
                        search_state.add_resource(&resource, &conn).unwrap();
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });

    store.clear_all_danger().unwrap();
}

#[cfg(feature = "db")]
criterion_group!(search_benches, search_benchmarks);

// PERFORMANCE OPTIMIZATION BENCHMARKS
// These benchmarks validate the performance improvements from our optimization plan

#[cfg(feature = "db")]
fn benchmark_optimizations(c: &mut Criterion) {
    let store = Db::init_temp("benchmark_optimizations").unwrap();
    let search_state = search_sqlite::SqliteSearchState::new(store.clone()).unwrap();
    
    // Add some test data for realistic benchmarks
    for i in 0..100 {
        let mut resource = Resource::new_generate_subject(&store).unwrap();
        resource.set_unsafe(
            urls::DESCRIPTION.into(),
            Value::Markdown(format!("Test content with search terms example {}", i)),
        );
        let conn = store.get_connection().unwrap();
        search_state.add_resource(&resource, &conn).unwrap();
    }
    
    // Benchmark 1: Fuzzy Search Optimization (Priority 1)
    c.bench_function("optimized/fuzzy_search_batch", |b| {
        b.iter(|| {
            search_state.fuzzy_search(black_box("example"), black_box(2), black_box(10)).unwrap()
        })
    });
    
    // Benchmark 2: Fuzzy Search with Complex Query (tests string sanitization indirectly)
    c.bench_function("optimized/fuzzy_search_complex", |b| {
        b.iter(|| {
            // Test with complex query containing special characters
            search_state.fuzzy_search(black_box("https://example"), black_box(2), black_box(5)).unwrap()
        })
    });
    
    // Benchmark 3: Serialization Optimization (Priority 3)
    c.bench_function("optimized/propvals_to_json_ad_map", |b| {
        let mut propvals = PropVals::new();
        for i in 0..10 {
            let subject = format!("https://example.com/resource{}", i);
            propvals.insert(
                subject.into(),
                Value::Markdown(format!("Test content {}", i)),
            );
        }
        
        b.iter(|| {
            serialize::propvals_to_json_ad_map(black_box(&propvals), black_box(None)).unwrap()
        })
    });
    
    // Benchmark 4: Combined Search Performance
    c.bench_function("optimized/text_search", |b| {
        b.iter(|| {
            search_state.text_search(black_box("content"), black_box(10)).unwrap()
        })
    });
    
    // Benchmark 5: Atomic Operations Performance (AtomicU64 optimization)
    c.bench_function("optimized/atomic_operations", |b| {
        use std::sync::atomic::{AtomicU64, Ordering};
        let counter = AtomicU64::new(0);
        
        b.iter(|| {
            counter.fetch_add(1, Ordering::SeqCst);
            black_box(counter.load(Ordering::SeqCst));
        })
    });
}

#[cfg(feature = "db")]
criterion_group!(benches, criterion_benchmark, search_benchmarks, benchmark_optimizations);

#[cfg(not(feature = "db"))]
criterion_group!(benches, criterion_benchmark);

criterion_main!(benches);
