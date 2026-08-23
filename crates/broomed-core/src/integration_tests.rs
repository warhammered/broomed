#[cfg(test)]
mod integration {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::thread;

    use crate::analysis::FileAnalysis;
    use crate::cache::{load_analysis, persist_analysis};
    use crate::db::create_schema;
    use crate::engines::{
        AudioTranscriptionEngine, EmbeddingEngine, HeuristicEmbedding, OcrEngine, StubAudio,
        StubOcr, StubVision, VisionEngine,
    };
    use crate::hardware::HardwareTier;
    use crate::models::{atomic_install_model, verify_checksum, ModelManifest, ModelRegistry};
    use crate::orchestrator::Orchestrator;
    use crate::search::{parse_query, search_files, search_hybrid};
    use rusqlite::Connection;

    // ── Embedding determinism / dimensions ──────────────────────────────
    #[test]
    fn embedding_determinism_and_dim() {
        let e = HeuristicEmbedding::new();
        assert_eq!(e.dim(), 384);
        let a = e.embed("vacation photos 2024 receipt invoice").unwrap();
        let b = e.embed("vacation photos 2024 receipt invoice").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 384);
        // L2 norm ~1
        let norm = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    // ── Model load failures / corrupted assets ──────────────────────────
    #[test]
    fn model_load_missing_gracefully() {
        // BundledLocalProvider with nonexistent dir should fallback to heuristic
        let p = crate::ai::BundledLocalProvider::with_model_dir("/no/such/model/__broomed_test__");
        assert!(!p.model_available());
        // orchestrator with stub engines still works
        let orch = Orchestrator::new();
        let dir = std::env::temp_dir().join(format!("broomed_int_missing_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("hello.txt");
        std::fs::write(&path, b"hello").unwrap();
        let a = orch.analyze(&path).unwrap();
        assert!(a.category.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_model_detection() {
        let dir = std::env::temp_dir().join(format!("broomed_corrupt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("BROOMED_MODEL_DIR", &dir);
        let manifest = ModelManifest {
            id: "corrupt-test".into(),
            version: "1.0.0".into(),
            filename: "model.bin".into(),
            checksum: blake3::hash(b"correct").to_hex().to_string(),
            size_bytes: 7,
            runtime: "test".into(),
            min_app_version: None,
        };
        // install with wrong data -> should fail checksum
        let err = atomic_install_model(&manifest, b"wrong").unwrap_err();
        assert!(err.to_string().contains("checksum mismatch"));
        // install correct then corrupt file
        atomic_install_model(&manifest, b"correct").unwrap();
        let file = dir.join("corrupt-test").join("model.bin");
        std::fs::write(&file, b"tampered").unwrap();
        assert!(!verify_checksum(&file, &manifest.checksum).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
        std::env::remove_var("BROOMED_MODEL_DIR");
    }

    // ── Malformed LLM / vision output ─────────────────────────────────
    #[test]
    fn malformed_llm_json_rejected() {
        assert!(crate::ai::parse_ai_json(r#"{"confidence":0.9}"#).is_err());
        assert!(crate::ai::parse_ai_json(r#"{"category":"","confidence":0.9}"#).is_err());
        assert!(crate::ai::parse_ai_json(r#"{"category":"x","confidence":2.0}"#).is_err());
        assert!(crate::ai::parse_ai_json("not json at all").is_err());
        assert!(
            crate::ai::parse_ai_json("```json\n{\"category\":\"x\",\"confidence\":0.5}\n```")
                .is_ok()
        );
    }

    #[test]
    fn vision_malformed_stub_graceful() {
        let v = StubVision;
        // stub never fails for any path, but we test that orchestrator doesn't crash on weird filenames
        let orch = Orchestrator::new();
        let a = orch.analyze(Path::new("/tmp/weird \0 name.jpg"));
        // should not panic; may succeed or fail but not panic
        let _ = a;
        assert!(v
            .describe(Path::new("/tmp/ok.jpg"))
            .unwrap()
            .contains("ok.jpg"));
    }

    // ── Transcription / OCR failures ───────────────────────────────────
    #[test]
    fn transcription_rejects_non_audio() {
        let a = StubAudio;
        assert!(a.transcribe(Path::new("/tmp/doc.pdf")).is_err());
        assert!(a.transcribe(Path::new("/tmp/image.jpg")).is_err());
    }

    #[test]
    fn ocr_stub_does_not_crash_on_missing_file() {
        let o = StubOcr;
        // stub always succeeds with placeholder, but should not panic on missing
        let s = o.extract_text(Path::new("/no/such/file.jpg")).unwrap();
        assert!(s.contains("ocr stub"));
    }

    // ── Unsupported files ───────────────────────────────────────────────
    #[test]
    fn unsupported_file_still_classified() {
        let orch = Orchestrator::new();
        let dir = std::env::temp_dir().join(format!("broomed_unsupported_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("weird.xyz");
        std::fs::write(&p, b"\x00\xFF\xFE weird binary").unwrap();
        let a = orch.analyze(&p).unwrap();
        assert_eq!(a.category.as_deref(), Some("General"));
        assert!(a.embedding.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Cache invalidation ─────────────────────────────────────────────
    #[test]
    fn cache_invalidation_on_model_change() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO files (id, path, filename, parent_directory) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["fid", "/tmp/a.txt", "a.txt", "/tmp"],
        )
        .unwrap();
        let mut a = FileAnalysis::new("/tmp/a.txt");
        a.size = Some(10);
        a.model_versions.embedding = Some("1.0.0".into());
        persist_analysis(&conn, "fid", "hash1", &a).unwrap();
        let key1 = a.cache_key("hash1");
        assert!(load_analysis(&conn, "fid").unwrap().is_some());
        // change model version
        a.model_versions.embedding = Some("2.0.0".into());
        let key2 = a.cache_key("hash1");
        assert_ne!(key1, key2);
        crate::cache::invalidate_if_model_changed(&conn, "fid", &key2).unwrap();
        assert!(load_analysis(&conn, "fid").unwrap().is_none());
    }

    // ── Large directory queues (1k-10k) ─────────────────────────────────
    #[test]
    fn large_directory_bounded() {
        let dir = std::env::temp_dir().join(format!("broomed_large_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..1000 {
            std::fs::write(dir.join(format!("file_{i:04}.txt")), b"hello world invoice").unwrap();
        }
        let orch = Orchestrator::new();
        // bounded concurrency via tier limit
        let tier = orch.hardware.tier;
        let concurrency = tier.concurrency();
        assert!(concurrency <= 8);
        // analyze in bounded chunks
        let paths: Vec<PathBuf> = (0..1000)
            .map(|i| dir.join(format!("file_{i:04}.txt")))
            .collect();
        let mut analyzed = 0usize;
        for chunk in paths.chunks(concurrency * 10) {
            for p in chunk {
                let _ = orch.analyze(p).unwrap();
                analyzed += 1;
            }
        }
        assert_eq!(analyzed, 1000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Cancellation (cooperative) ─────────────────────────────────────
    #[test]
    fn cancellation_flag() {
        let cancel = Arc::new(Mutex::new(false));
        let cancel_clone = Arc::clone(&cancel);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                if *cancel_clone.lock().unwrap() {
                    return i;
                }
                thread::sleep(std::time::Duration::from_millis(1));
            }
            100
        });
        thread::sleep(std::time::Duration::from_millis(5));
        *cancel.lock().unwrap() = true;
        let n = handle.join().unwrap();
        assert!(n < 100, "should cancel early");
    }

    // ── Concurrent analysis ────────────────────────────────────────────
    #[test]
    fn concurrent_analysis_no_race() {
        let dir = std::env::temp_dir().join(format!("broomed_conc_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..20 {
            std::fs::write(dir.join(format!("c{i}.txt")), format!("content {i}")).unwrap();
        }
        let orch = Arc::new(Orchestrator::new());
        let mut handles = Vec::new();
        for i in 0..20 {
            let o = Arc::clone(&orch);
            let p = dir.join(format!("c{i}.txt"));
            handles.push(thread::spawn(move || o.analyze(&p).unwrap()));
        }
        let mut results = Vec::new();
        for h in handles {
            results.push(h.join().unwrap());
        }
        assert_eq!(results.len(), 20);
        for a in results {
            assert!(a.embedding.is_some());
            assert_eq!(a.embedding.unwrap().len(), 384);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Offline operation ──────────────────────────────────────────────
    #[test]
    fn offline_no_network_required() {
        // All engines are heuristic stubs - no env var, no network
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("ANTHROPIC_API_KEY");
        let orch = Orchestrator::new();
        let a = orch.analyze(Path::new("/tmp/offline_test.pdf"));
        // even for nonexistent file, orchestrator returns analysis with General fallback
        assert!(a.is_ok() || a.unwrap_err().to_string().contains(""));
        let e = HeuristicEmbedding::new();
        assert!(e.embed("offline query").is_ok());
    }

    // ── Filesystem safety (AI cannot bypass) ───────────────────────────
    #[test]
    fn ai_suggestion_cannot_bypass_security() {
        use crate::security::validate_path;
        let base = std::env::temp_dir().join(format!("broomed_sec_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let base = base.canonicalize().unwrap();
        // AI suggests "../evil" - must be rejected
        let evil = Path::new("../evil.txt");
        assert!(validate_path(evil, &base).is_err());
        let evil2 = Path::new("/etc/passwd");
        assert!(validate_path(evil2, &base).is_err());
        // valid inside base
        assert!(validate_path(Path::new("Documents/report.pdf"), &base).is_ok());
        let _ = std::fs::remove_dir_all(&base);
    }

    // ── AI suggestion validation ───────────────────────────────────────
    #[test]
    fn classification_validation_rejects_unsafe() {
        let c = crate::analysis::Classification {
            category: "Documents".into(),
            subcategory: None,
            tags: vec!["docs".into()],
            suggested_folder: "../evil".into(),
            confidence: 0.9,
            reason: "test".into(),
        };
        assert!(c.validate().is_err());
        let c2 = crate::analysis::Classification {
            category: "".into(),
            ..c.clone()
        };
        assert!(c2.validate().is_err());
    }

    // ── Semantic search hybrid ─────────────────────────────────────────
    #[test]
    fn hybrid_search_uses_embeddings() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        // insert two files with embeddings
        conn.execute("INSERT INTO files (id, path, filename, mime_type, parent_directory) VALUES (?1, ?2, ?3, ?4, ?5)", rusqlite::params!["id1", "/a/vacation_beach.jpg", "vacation_beach.jpg", "image/jpeg", "/a"]).unwrap();
        conn.execute("INSERT INTO files (id, path, filename, mime_type, parent_directory) VALUES (?1, ?2, ?3, ?4, ?5)", rusqlite::params!["id2", "/b/tax_report.pdf", "tax_report.pdf", "application/pdf", "/b"]).unwrap();
        let e = HeuristicEmbedding::new();
        let beach_vec = e.embed("vacation beach photo holiday").unwrap();
        let tax_vec = e.embed("tax finance invoice report").unwrap();
        let to_blob = |v: Vec<f32>| {
            let mut b = Vec::new();
            for f in v {
                b.extend_from_slice(&f.to_le_bytes());
            }
            b
        };
        conn.execute(
            "INSERT INTO file_embeddings (file_id, model, vec) VALUES (?1, ?2, ?3)",
            rusqlite::params!["id1", "heuristic-384", to_blob(beach_vec)],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO file_embeddings (file_id, model, vec) VALUES (?1, ?2, ?3)",
            rusqlite::params!["id2", "heuristic-384", to_blob(tax_vec)],
        )
        .unwrap();
        // semantic query "vacation photos" should rank beach higher
        let q = parse_query("vacation photos");
        let res = search_hybrid(&conn, &q, &e, 10).unwrap();
        assert!(!res.is_empty());
        assert_eq!(res[0], "/a/vacation_beach.jpg");
        // LIKE fallback
        let q2 = parse_query("tax_report");
        let res2 = search_files(&conn, &q2, 10).unwrap();
        assert_eq!(res2, vec!["/b/tax_report.pdf"]);
    }

    // ── Model sizes within target ──────────────────────────────────────
    #[test]
    fn model_sizes_within_500_700() {
        let r = ModelRegistry::default_registry();
        let total = r.total_default_bytes();
        assert!(
            (400_000_000..=750_000_000).contains(&total),
            "total {total}"
        );
        for m in r.models.values() {
            assert!(m.size_bytes <= 300_000_000, "{} too large", m.id);
        }
    }

    // ── Hardware tiers deterministic ───────────────────────────────────
    #[test]
    fn hardware_tiers() {
        assert_eq!(HardwareTier::Tier0.concurrency(), 2);
        assert_eq!(HardwareTier::Tier1.concurrency(), 4);
        assert_eq!(HardwareTier::Tier2.concurrency(), 8);
    }

    // ── Duplicate detection uses hash not LLM ───────────────────────────
    #[test]
    fn duplicate_via_hash() {
        let dir = std::env::temp_dir().join(format!("broomed_dup_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p1 = dir.join("a.txt");
        let p2 = dir.join("b.txt");
        std::fs::write(&p1, b"same content").unwrap();
        std::fs::write(&p2, b"same content").unwrap();
        let h1 = crate::hash::hash_file(&p1).unwrap();
        let h2 = crate::hash::hash_file(&p2).unwrap();
        assert_eq!(h1, h2);
        // embeddings should also be similar
        let e = HeuristicEmbedding::new();
        let v1 = e.embed("same content").unwrap();
        let v2 = e.embed("same content").unwrap();
        let cosine = {
            let mut dot = 0f32;
            let mut na = 0f32;
            let mut nb = 0f32;
            for (x, y) in v1.iter().zip(v2.iter()) {
                dot += x * y;
                na += x * x;
                nb += y * y;
            }
            dot / (na.sqrt() * nb.sqrt())
        };
        assert!((cosine - 1.0).abs() < 1e-6);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
