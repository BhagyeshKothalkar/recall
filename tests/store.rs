use std::{fs, path::PathBuf};

use recall::{
    application::{
        ports::{JobRepository, MemoryRepository},
        store::{MemoryLimits, StoreInput, StoreMemory},
    },
    domain::JobState,
    infrastructure::{
        clock::SystemClock,
        database::{SqliteJobRepository, SqliteMemoryRepository, SqliteStorePersistence},
    },
};

fn temporary_database() -> PathBuf {
    std::env::temp_dir().join(format!(
        "recall-store-integration-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

#[test]
fn store_service_persists_memory_and_pending_job() {
    let path = temporary_database();
    let persistence = SqliteStorePersistence::open(&path).unwrap();
    let store = StoreMemory::new(persistence, SystemClock, MemoryLimits::new(1024));

    let response = store
        .execute(StoreInput::Text("integration memory".to_owned()))
        .unwrap();

    let memories = SqliteMemoryRepository::open(&path).unwrap();
    assert_eq!(
        memories
            .get(response.memory_id())
            .unwrap()
            .unwrap()
            .content(),
        "integration memory"
    );

    let jobs = SqliteJobRepository::open(&path).unwrap();
    let pending = jobs.list_by_state(JobState::Pending).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].kind(),
        recall::domain::JobKind::GenerateEmbedding {
            memory_id: response.memory_id()
        }
    );

    drop(jobs);
    drop(memories);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(path.with_extension("db-wal"));
    let _ = fs::remove_file(path.with_extension("db-shm"));
}
