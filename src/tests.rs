use super::*;
use shikifsrs_elden_ring::{
    gparam::NativeGparamCall,
    gparam_task::{GparamPreloader, GparamSubmissionState, GparamTaskBackend},
    objects::{ObjectKind, ObjectObservation},
};
use shikifsrs_runtime::{Error, Result};

#[derive(Default)]
struct Backend {
    calls: Vec<(char, u16)>,
    fail_request: u32,
}
// SAFETY: Original contract fixture records copied IDs only, with no game/Win32/memory calls.
unsafe impl GparamTaskBackend for Backend {
    fn elapsed_ms(&self) -> Result<u64> {
        Ok(0)
    }
    unsafe fn request(&mut self, id: CommonEventId) -> Result<NativeGparamCall<Option<u64>>> {
        self.calls.push(('r', id.value()));
        if self.fail_request > 0 {
            self.fail_request -= 1;
            return Err(Error::NativeOperationRejected(
                "fixture preflight missing manager",
            ));
        }
        Ok(NativeGparamCall {
            manager: ObjectObservation {
                address: 0x1000,
                kind: ObjectKind::FileResourceManager,
            },
            id,
            value: Some(0x2000),
            postflight: Ok(()),
        })
    }
    unsafe fn prime(&mut self, id: CommonEventId) -> Result<NativeGparamCall<()>> {
        self.calls.push(('p', id.value()));
        Ok(NativeGparamCall {
            manager: ObjectObservation {
                address: 0x3000,
                kind: ObjectKind::WorldSceneDrawParameters,
            },
            id,
            value: (),
            postflight: Ok(()),
        })
    }
}
fn tick(preloader: &mut GparamPreloader, backend: &mut Backend, now: u64) -> bool {
    // SAFETY: Synthetic original backend never dereferences or invokes native addresses.
    unsafe { preloader.tick(now, backend) }.unwrap()
}

#[test]
fn owned_default_and_shipped_ini_keep_all_controls() {
    let default = Config::default().sdk_config().unwrap();
    assert!(default.enabled && default.request_filecap && default.prime_drawparam);
    assert_eq!(
        default.ids.iter().map(|id| id.value()).collect::<Vec<_>>(),
        [5]
    );
    assert_eq!(
        (
            default.start_delay_ms,
            default.attempts_per_id,
            default.retry_every_frames
        ),
        (5000, 120, 60)
    );
    assert!(Config::default().log_enabled);
    let shipped = Config::parse(include_str!("../config/gparam_preload_patch.ini"));
    assert!(!shipped.log_enabled);
    assert_eq!(shipped.common_event_ids, [5, 6, 7, 8, 21, 25]);
    assert_eq!(shipped.sdk_config().unwrap().ids.len(), 6);
}

#[test]
fn original_aliases_comments_sorting_and_minimum_one_remain_supported() {
    let config = Config::parse(
        "enabled=YES # comment\nenable_log=off\nwrite_log=on\ncommon_event_ids=21,5 5 bad,6\ndirect_preload_delay_ms=17\ndirect_preload_retries=0\nretry_every_frames=0\ndirect_preload=no\nrequest_filecap=0",
    );
    assert!(config.enabled && config.log_enabled);
    assert_eq!(config.common_event_ids, [5, 6, 21]);
    let native = config.sdk_config().unwrap();
    assert_eq!(
        (
            native.start_delay_ms,
            native.attempts_per_id,
            native.retry_every_frames
        ),
        (17, 1, 1)
    );
    assert!(!native.prime_drawparam && !native.request_filecap);
    assert_eq!(
        Config::parse("common_event_ids=5,6\ndirect_preload_id=7").common_event_ids,
        [7]
    );
    assert!(Config::parse("log_enabled=invalid\nenabled=invalid").enabled);
    assert!(!Config::parse("enabled=false").sdk_config().unwrap().enabled);
    assert!(
        Config::parse("common_event_ids=")
            .common_event_ids
            .is_empty()
    );
}

#[test]
fn native_key_validation_rejects_mismatched_paths_and_unbounded_id_count() {
    let config = Config::parse("common_event_ids=0,65535");
    let ids = config.sdk_config().unwrap().ids;
    assert_eq!(ids[0].path(), "gparam:/m00_00_0000_CommonEvent.gparam");
    assert_eq!(ids[1].path(), "gparam:/m00_00_65535_CommonEvent.gparam");
    assert!(
        Config::parse("common_event_ids=65536")
            .sdk_config()
            .is_err()
    );
    assert!(
        Config::parse("common_event_ids=4294967295")
            .sdk_config()
            .is_err()
    );
    let many = (0..257)
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    assert!(
        Config::parse(&format!("common_event_ids={many}"))
            .sdk_config()
            .is_err()
    );
    assert!(Ww171File::verify(b"owned fixture: unknown executable").is_err());
}

#[test]
fn configured_delay_and_cadence_one_actually_submit_each_id_once() {
    let config = Config::parse("common_event_ids=6,5,6\nstart_delay_ms=5\nretry_every_frames=1")
        .sdk_config()
        .unwrap();
    let mut preloader = GparamPreloader::new(config, 0).unwrap();
    let mut backend = Backend::default();
    assert!(!tick(&mut preloader, &mut backend, 4));
    assert!(backend.calls.is_empty());
    assert!(tick(&mut preloader, &mut backend, 5));
    assert_eq!(backend.calls, [('r', 5), ('p', 5), ('r', 6), ('p', 6)]);
    assert!(preloader.settled());
    assert!(!tick(&mut preloader, &mut backend, 6));
    assert_eq!(backend.calls.len(), 4);
}

#[test]
fn consumer_retry_budget_and_frame_interval_reach_exhaustion() {
    let config = Config::parse(
        "common_event_ids=5\nstart_delay_ms=0\nretries_per_id=2\nretry_every_frames=3",
    )
    .sdk_config()
    .unwrap();
    let mut preloader = GparamPreloader::new(config, 0).unwrap();
    let mut backend = Backend {
        fail_request: 3,
        ..Backend::default()
    };
    assert!(tick(&mut preloader, &mut backend, 0));
    for _ in 0..2 {
        assert!(!tick(&mut preloader, &mut backend, 0));
    }
    assert!(tick(&mut preloader, &mut backend, 0));
    assert_eq!(backend.calls, [('r', 5), ('r', 5)]);
    assert_eq!(preloader.submissions()[0].attempts, 2);
    assert_eq!(
        preloader.submissions()[0].state,
        GparamSubmissionState::Exhausted
    );
    assert!(!tick(&mut preloader, &mut backend, 0));
}

#[test]
fn independent_request_and_prime_switches_cover_all_four_configurations() {
    for (request, prime) in [(false, false), (false, true), (true, false), (true, true)] {
        let config = Config::parse(&format!(
            "start_delay_ms=0\nrequest_filecap={request}\nprime_drawparam={prime}"
        ))
        .sdk_config()
        .unwrap();
        let mut preloader = GparamPreloader::new(config, 0).unwrap();
        let mut backend = Backend::default();
        tick(&mut preloader, &mut backend, 0);
        let mut expected = Vec::new();
        if request {
            expected.push(('r', 5));
        }
        if prime {
            expected.push(('p', 5));
        }
        assert_eq!(backend.calls, expected);
        assert!(preloader.settled());
    }
}
