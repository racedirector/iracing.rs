use std::{sync::Arc, time::Duration};

use iracing_sdk::{
    BroadcastCommand, CameraState, PitCommand, ReplayPositionMode, ReplaySearchMode,
    ReplayStateMode, TelemetryCommandMode, VideoCaptureMode,
};

use super::{
    AvailableCameras, BroadcastCommandPort, BroadcastError, CameraSelectionExpectation,
    CameraSelectionSnapshot, CameraStateExpectation, CameraStatePort, CameraStateSnapshot,
    ForceFeedbackExpectation, ForceFeedbackSnapshot, ForceFeedbackStatePort, PitServiceExpectation,
    PitServiceSnapshot, PitStatePort, ReplayPositionExpectation, ReplayPositionSnapshot,
    ReplaySpeedExpectation, ReplaySpeedSnapshot, ReplayStatePort, TelemetryLoggingExpectation,
    TelemetryLoggingSnapshot, TelemetryStatePort,
};

pub(crate) struct BroadcastUseCases {
    commands: Arc<dyn BroadcastCommandPort>,
    camera: Arc<dyn CameraStatePort>,
    replay: Arc<dyn ReplayStatePort>,
    pit: Arc<dyn PitStatePort>,
    telemetry: Arc<dyn TelemetryStatePort>,
    force_feedback: Arc<dyn ForceFeedbackStatePort>,
    observation_timeout: Duration,
}

impl BroadcastUseCases {
    pub(crate) fn new(
        commands: Arc<dyn BroadcastCommandPort>,
        camera: Arc<dyn CameraStatePort>,
        replay: Arc<dyn ReplayStatePort>,
        pit: Arc<dyn PitStatePort>,
        telemetry: Arc<dyn TelemetryStatePort>,
        force_feedback: Arc<dyn ForceFeedbackStatePort>,
        observation_timeout: Duration,
    ) -> Self {
        Self {
            commands,
            camera,
            replay,
            pit,
            telemetry,
            force_feedback,
            observation_timeout,
        }
    }

    async fn send(&self, command: BroadcastCommand) -> Result<(), BroadcastError> {
        self.commands.send(command).await
    }

    pub(crate) async fn get_available_cameras(&self) -> Result<AvailableCameras, BroadcastError> {
        let current = self.camera.selection_snapshot().await?;
        let camera_groups = self
            .camera
            .available_camera_groups(current.session_version)
            .await?;

        Ok(AvailableCameras {
            camera_groups,
            current,
        })
    }

    pub(crate) async fn camera_switch_position(
        &self,
        position: u16,
        group: u16,
        camera: u16,
    ) -> Result<CameraSelectionSnapshot, BroadcastError> {
        let previous = self.camera.selection_snapshot().await?;

        self.send(BroadcastCommand::CameraSwitchPosition(
            position, group, camera,
        ))
        .await?;

        self.camera
            .wait_for_selection(
                previous,
                CameraSelectionExpectation {
                    car_index: None,
                    group: u32::from(group),
                    camera: u32::from(camera),
                },
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn camera_switch_number(
        &self,
        car_number: String,
        group: u16,
        camera: u16,
    ) -> Result<CameraSelectionSnapshot, BroadcastError> {
        let previous = self.camera.selection_snapshot().await?;
        let car_index = self
            .camera
            .resolve_car_index_by_number(previous.session_version, &car_number)
            .await?;

        self.send(BroadcastCommand::CameraSwitchNumber(
            car_number, group, camera,
        ))
        .await?;

        self.camera
            .wait_for_selection(
                previous,
                CameraSelectionExpectation {
                    car_index: Some(car_index),
                    group: u32::from(group),
                    camera: u32::from(camera),
                },
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn camera_set_state(
        &self,
        state: CameraState,
    ) -> Result<CameraStateSnapshot, BroadcastError> {
        let previous = self.camera.state_snapshot().await?;

        self.send(BroadcastCommand::CameraSetState(state)).await?;

        self.camera
            .wait_for_state(
                previous,
                CameraStateExpectation {
                    state: state.bits(),
                },
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn replay_set_play_speed(
        &self,
        speed: i16,
        is_slow_motion: bool,
    ) -> Result<ReplaySpeedSnapshot, BroadcastError> {
        let previous = self.replay.speed_snapshot().await?;

        self.send(BroadcastCommand::ReplaySetPlaySpeed(speed, is_slow_motion))
            .await?;

        self.replay
            .wait_for_speed(
                previous,
                ReplaySpeedExpectation {
                    speed: i32::from(speed),
                    is_slow_motion,
                },
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn replay_set_play_position(
        &self,
        mode: ReplayPositionMode,
        frame: u32,
    ) -> Result<ReplayPositionSnapshot, BroadcastError> {
        let previous = self.replay.position_snapshot().await?;

        self.send(BroadcastCommand::ReplaySetPlayPosition(mode, frame))
            .await?;

        self.replay
            .wait_for_position(
                previous,
                ReplayPositionExpectation::AnyChange,
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn replay_search(
        &self,
        mode: ReplaySearchMode,
    ) -> Result<ReplayPositionSnapshot, BroadcastError> {
        let previous = self.replay.position_snapshot().await?;

        self.send(BroadcastCommand::ReplaySearch(mode)).await?;

        self.replay
            .wait_for_position(
                previous,
                ReplayPositionExpectation::AnyChange,
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn replay_set_state(
        &self,
        state: ReplayStateMode,
    ) -> Result<(), BroadcastError> {
        self.send(BroadcastCommand::ReplaySetState(state)).await
    }

    pub(crate) async fn reload_textures(&self, car_idx: Option<u16>) -> Result<(), BroadcastError> {
        self.send(match car_idx {
            Some(index) => BroadcastCommand::ReloadTextures(index),
            None => BroadcastCommand::ReloadAllTextures,
        })
        .await
    }

    pub(crate) async fn chat_command(
        &self,
        command: BroadcastCommand,
    ) -> Result<(), BroadcastError> {
        debug_assert!(matches!(
            command,
            BroadcastCommand::ChatCommand(_) | BroadcastCommand::ChatCommandMacro(_)
        ));
        self.send(command).await
    }

    pub(crate) async fn pit_command(
        &self,
        command: PitCommand,
    ) -> Result<PitServiceSnapshot, BroadcastError> {
        let previous = self.pit.pit_service_snapshot().await?;

        self.send(BroadcastCommand::PitCommand(command)).await?;

        self.pit
            .wait_for_pit_service(
                previous,
                PitServiceExpectation::AnyChange,
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn pit_command_stream(
        &self,
        commands: Vec<PitCommand>,
    ) -> Result<PitServiceSnapshot, BroadcastError> {
        let previous = self.pit.pit_service_snapshot().await?;

        if commands.is_empty() {
            return Ok(previous);
        }

        for command in commands {
            self.send(BroadcastCommand::PitCommand(command)).await?;
        }

        self.pit
            .wait_for_pit_service(
                previous,
                PitServiceExpectation::AnyChange,
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn telemetry_command(
        &self,
        mode: TelemetryCommandMode,
    ) -> Result<TelemetryLoggingSnapshot, BroadcastError> {
        let previous = self.telemetry.logging_snapshot().await?;

        self.send(BroadcastCommand::TelemetryCommand(mode)).await?;

        self.telemetry
            .wait_for_logging(
                previous,
                TelemetryLoggingExpectation {
                    is_disk_logging_enabled: mode != TelemetryCommandMode::Stop,
                },
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn force_feedback_command(
        &self,
        max_force: f32,
    ) -> Result<ForceFeedbackSnapshot, BroadcastError> {
        let previous = self.force_feedback.force_feedback_snapshot().await?;

        self.send(BroadcastCommand::FFBCommand(max_force)).await?;

        self.force_feedback
            .wait_for_force_feedback(
                previous,
                ForceFeedbackExpectation { max_force },
                self.observation_timeout,
            )
            .await
    }

    pub(crate) async fn replay_search_session_time(
        &self,
        session_number: u16,
        session_time_ms: u32,
    ) -> Result<(), BroadcastError> {
        self.send(BroadcastCommand::ReplaySearchSessionTime(
            session_number,
            session_time_ms,
        ))
        .await
    }

    pub(crate) async fn video_capture(&self, mode: VideoCaptureMode) -> Result<(), BroadcastError> {
        self.send(BroadcastCommand::VideoCapture(mode)).await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex as StdMutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use iracing_sdk::{CameraState, ChatCommandMode, PitCommand, ReplayPositionMode};

    use super::*;
    use crate::broadcast_app::AvailableCameraGroup;

    #[derive(Debug, Clone, PartialEq)]
    enum Event {
        CameraSelectionSnapshot,
        CameraStateSnapshot,
        Resolve {
            session_version: u32,
            car_number: String,
        },
        Send(BroadcastCommand),
        CameraSelectionWait {
            previous: CameraSelectionSnapshot,
            expected: CameraSelectionExpectation,
            timeout: Duration,
        },
        CameraStateWait {
            previous: CameraStateSnapshot,
            expected: CameraStateExpectation,
            timeout: Duration,
        },
        ReplaySpeedSnapshot,
        ReplayPositionSnapshot,
        ReplaySpeedWait {
            previous: ReplaySpeedSnapshot,
            expected: ReplaySpeedExpectation,
            timeout: Duration,
        },
        ReplayPositionWait {
            previous: ReplayPositionSnapshot,
            expected: ReplayPositionExpectation,
            timeout: Duration,
        },
        PitSnapshot,
        PitWait {
            previous: PitServiceSnapshot,
            expected: PitServiceExpectation,
            timeout: Duration,
        },
        TelemetrySnapshot,
        TelemetryWait {
            previous: TelemetryLoggingSnapshot,
            expected: TelemetryLoggingExpectation,
            timeout: Duration,
        },
        ForceFeedbackSnapshot,
        ForceFeedbackWait {
            previous: ForceFeedbackSnapshot,
            expected: ForceFeedbackExpectation,
            timeout: Duration,
        },
    }

    #[derive(Default)]
    struct FakeCommands {
        events: Arc<StdMutex<Vec<Event>>>,
        error: StdMutex<Option<BroadcastError>>,
    }

    #[async_trait]
    impl BroadcastCommandPort for FakeCommands {
        async fn send(&self, command: BroadcastCommand) -> Result<(), BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::Send(command));

            if let Some(error) = self.error.lock().expect("command mutex poisoned").take() {
                return Err(error);
            }

            Ok(())
        }
    }

    struct FakeCamera {
        events: Arc<StdMutex<Vec<Event>>>,
        selection_snapshot: CameraSelectionSnapshot,
        selection_wait: StdMutex<Option<Result<CameraSelectionSnapshot, BroadcastError>>>,
        state_snapshot: CameraStateSnapshot,
        state_wait: StdMutex<Option<Result<CameraStateSnapshot, BroadcastError>>>,
        resolutions: StdMutex<HashMap<String, u32>>,
    }

    #[async_trait]
    impl CameraStatePort for FakeCamera {
        async fn selection_snapshot(&self) -> Result<CameraSelectionSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::CameraSelectionSnapshot);
            Ok(self.selection_snapshot)
        }

        async fn wait_for_selection(
            &self,
            previous: CameraSelectionSnapshot,
            expected: CameraSelectionExpectation,
            timeout: Duration,
        ) -> Result<CameraSelectionSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::CameraSelectionWait {
                    previous,
                    expected,
                    timeout,
                });
            self.selection_wait
                .lock()
                .expect("camera mutex poisoned")
                .take()
                .expect("camera selection wait result should be configured")
        }

        async fn state_snapshot(&self) -> Result<CameraStateSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::CameraStateSnapshot);
            Ok(self.state_snapshot)
        }

        async fn wait_for_state(
            &self,
            previous: CameraStateSnapshot,
            expected: CameraStateExpectation,
            timeout: Duration,
        ) -> Result<CameraStateSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::CameraStateWait {
                    previous,
                    expected,
                    timeout,
                });
            self.state_wait
                .lock()
                .expect("camera mutex poisoned")
                .take()
                .expect("camera state wait result should be configured")
        }

        async fn available_camera_groups(
            &self,
            _session_version: u32,
        ) -> Result<Vec<AvailableCameraGroup>, BroadcastError> {
            Ok(Vec::new())
        }

        async fn resolve_car_index_by_number(
            &self,
            session_version: u32,
            car_number: &str,
        ) -> Result<u32, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::Resolve {
                    session_version,
                    car_number: car_number.to_string(),
                });
            self.resolutions
                .lock()
                .expect("resolutions mutex poisoned")
                .get(car_number)
                .copied()
                .ok_or_else(|| BroadcastError::FailedPrecondition("missing car".to_string()))
        }
    }

    struct FakeReplay {
        events: Arc<StdMutex<Vec<Event>>>,
        speed_snapshot: ReplaySpeedSnapshot,
        speed_wait: StdMutex<Option<Result<ReplaySpeedSnapshot, BroadcastError>>>,
        position_snapshot: ReplayPositionSnapshot,
        position_wait: StdMutex<Option<Result<ReplayPositionSnapshot, BroadcastError>>>,
    }

    #[async_trait]
    impl ReplayStatePort for FakeReplay {
        async fn speed_snapshot(&self) -> Result<ReplaySpeedSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::ReplaySpeedSnapshot);
            Ok(self.speed_snapshot)
        }

        async fn wait_for_speed(
            &self,
            previous: ReplaySpeedSnapshot,
            expected: ReplaySpeedExpectation,
            timeout: Duration,
        ) -> Result<ReplaySpeedSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::ReplaySpeedWait {
                    previous,
                    expected,
                    timeout,
                });
            self.speed_wait
                .lock()
                .expect("replay mutex poisoned")
                .take()
                .expect("replay speed wait result should be configured")
        }

        async fn position_snapshot(&self) -> Result<ReplayPositionSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::ReplayPositionSnapshot);
            Ok(self.position_snapshot)
        }

        async fn wait_for_position(
            &self,
            previous: ReplayPositionSnapshot,
            expected: ReplayPositionExpectation,
            timeout: Duration,
        ) -> Result<ReplayPositionSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::ReplayPositionWait {
                    previous,
                    expected,
                    timeout,
                });
            self.position_wait
                .lock()
                .expect("replay mutex poisoned")
                .take()
                .expect("replay position wait result should be configured")
        }
    }

    struct FakePit {
        events: Arc<StdMutex<Vec<Event>>>,
        snapshot: PitServiceSnapshot,
        wait: StdMutex<Option<Result<PitServiceSnapshot, BroadcastError>>>,
    }

    #[async_trait]
    impl PitStatePort for FakePit {
        async fn pit_service_snapshot(&self) -> Result<PitServiceSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::PitSnapshot);
            Ok(self.snapshot)
        }

        async fn wait_for_pit_service(
            &self,
            previous: PitServiceSnapshot,
            expected: PitServiceExpectation,
            timeout: Duration,
        ) -> Result<PitServiceSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::PitWait {
                    previous,
                    expected,
                    timeout,
                });
            self.wait
                .lock()
                .expect("pit mutex poisoned")
                .take()
                .expect("pit wait result should be configured")
        }
    }

    struct FakeTelemetry {
        events: Arc<StdMutex<Vec<Event>>>,
        snapshot: TelemetryLoggingSnapshot,
        wait: StdMutex<Option<Result<TelemetryLoggingSnapshot, BroadcastError>>>,
    }

    #[async_trait]
    impl TelemetryStatePort for FakeTelemetry {
        async fn logging_snapshot(&self) -> Result<TelemetryLoggingSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::TelemetrySnapshot);
            Ok(self.snapshot)
        }

        async fn wait_for_logging(
            &self,
            previous: TelemetryLoggingSnapshot,
            expected: TelemetryLoggingExpectation,
            timeout: Duration,
        ) -> Result<TelemetryLoggingSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::TelemetryWait {
                    previous,
                    expected,
                    timeout,
                });
            self.wait
                .lock()
                .expect("telemetry mutex poisoned")
                .take()
                .expect("telemetry wait result should be configured")
        }
    }

    struct FakeForceFeedback {
        events: Arc<StdMutex<Vec<Event>>>,
        snapshot: ForceFeedbackSnapshot,
        wait: StdMutex<Option<Result<ForceFeedbackSnapshot, BroadcastError>>>,
    }

    #[async_trait]
    impl ForceFeedbackStatePort for FakeForceFeedback {
        async fn force_feedback_snapshot(&self) -> Result<ForceFeedbackSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::ForceFeedbackSnapshot);
            Ok(self.snapshot)
        }

        async fn wait_for_force_feedback(
            &self,
            previous: ForceFeedbackSnapshot,
            expected: ForceFeedbackExpectation,
            timeout: Duration,
        ) -> Result<ForceFeedbackSnapshot, BroadcastError> {
            self.events
                .lock()
                .expect("events mutex poisoned")
                .push(Event::ForceFeedbackWait {
                    previous,
                    expected,
                    timeout,
                });
            self.wait
                .lock()
                .expect("force feedback mutex poisoned")
                .take()
                .expect("force feedback wait result should be configured")
        }
    }

    struct Fixture {
        events: Arc<StdMutex<Vec<Event>>>,
        use_cases: BroadcastUseCases,
    }

    fn camera_snapshot(car_index: u32, group: u32, camera: u32) -> CameraSelectionSnapshot {
        CameraSelectionSnapshot {
            session_version: 7,
            car_index,
            group,
            camera,
        }
    }

    fn camera_state(state: u32) -> CameraStateSnapshot {
        CameraStateSnapshot { state }
    }

    fn replay_speed(speed: i32, is_slow_motion: bool) -> ReplaySpeedSnapshot {
        ReplaySpeedSnapshot {
            speed,
            is_slow_motion,
        }
    }

    fn replay_position(
        frame: u32,
        session_number: u32,
        session_time: f32,
    ) -> ReplayPositionSnapshot {
        ReplayPositionSnapshot {
            frame,
            session_number,
            session_time,
        }
    }

    fn pit_snapshot(service_flags: u32) -> PitServiceSnapshot {
        PitServiceSnapshot {
            service_flags,
            fuel: 1.0,
            lf_pressure: 2.0,
            rf_pressure: 3.0,
            lr_pressure: 4.0,
            rr_pressure: 5.0,
            tire_compound: 6,
        }
    }

    fn telemetry_snapshot(enabled: bool, active: bool) -> TelemetryLoggingSnapshot {
        TelemetryLoggingSnapshot {
            is_disk_logging_enabled: enabled,
            is_disk_logging_active: active,
        }
    }

    fn fixture() -> Fixture {
        let events = Arc::new(StdMutex::new(Vec::new()));
        let commands = Arc::new(FakeCommands {
            events: Arc::clone(&events),
            error: StdMutex::new(None),
        });
        let camera = Arc::new(FakeCamera {
            events: Arc::clone(&events),
            selection_snapshot: camera_snapshot(1, 2, 3),
            selection_wait: StdMutex::new(Some(Ok(camera_snapshot(42, 4, 5)))),
            state_snapshot: camera_state(0),
            state_wait: StdMutex::new(Some(Ok(camera_state(CameraState::UI_HIDDEN.bits())))),
            resolutions: StdMutex::new(HashMap::from([("012".to_string(), 12)])),
        });
        let replay = Arc::new(FakeReplay {
            events: Arc::clone(&events),
            speed_snapshot: replay_speed(0, false),
            speed_wait: StdMutex::new(Some(Ok(replay_speed(2, true)))),
            position_snapshot: replay_position(10, 1, 2.0),
            position_wait: StdMutex::new(Some(Ok(replay_position(20, 1, 3.0)))),
        });
        let pit = Arc::new(FakePit {
            events: Arc::clone(&events),
            snapshot: pit_snapshot(0),
            wait: StdMutex::new(Some(Ok(pit_snapshot(1)))),
        });
        let telemetry = Arc::new(FakeTelemetry {
            events: Arc::clone(&events),
            snapshot: telemetry_snapshot(true, true),
            wait: StdMutex::new(Some(Ok(telemetry_snapshot(false, false)))),
        });
        let force_feedback = Arc::new(FakeForceFeedback {
            events: Arc::clone(&events),
            snapshot: ForceFeedbackSnapshot { max_force: 10.0 },
            wait: StdMutex::new(Some(Ok(ForceFeedbackSnapshot { max_force: 20.0 }))),
        });

        Fixture {
            events,
            use_cases: BroadcastUseCases::new(
                commands,
                camera,
                replay,
                pit,
                telemetry,
                force_feedback,
                Duration::from_millis(25),
            ),
        }
    }

    #[tokio::test]
    async fn camera_switch_position_snapshots_sends_then_waits() {
        let Fixture { events, use_cases } = fixture();

        let result = use_cases
            .camera_switch_position(42, 4, 5)
            .await
            .expect("switch should succeed");

        assert_eq!(result, camera_snapshot(42, 4, 5));
        assert_eq!(
            *events.lock().expect("events mutex poisoned"),
            vec![
                Event::CameraSelectionSnapshot,
                Event::Send(BroadcastCommand::CameraSwitchPosition(42, 4, 5)),
                Event::CameraSelectionWait {
                    previous: camera_snapshot(1, 2, 3),
                    expected: CameraSelectionExpectation {
                        car_index: None,
                        group: 4,
                        camera: 5,
                    },
                    timeout: Duration::from_millis(25),
                },
            ]
        );
    }

    #[tokio::test]
    async fn camera_switch_number_resolves_before_sending() {
        let Fixture { events, use_cases } = fixture();

        let result = use_cases
            .camera_switch_number("012".to_string(), 6, 7)
            .await
            .expect("switch should succeed");

        assert_eq!(result, camera_snapshot(42, 4, 5));
        assert_eq!(
            *events.lock().expect("events mutex poisoned"),
            vec![
                Event::CameraSelectionSnapshot,
                Event::Resolve {
                    session_version: 7,
                    car_number: "012".to_string(),
                },
                Event::Send(BroadcastCommand::CameraSwitchNumber(
                    "012".to_string(),
                    6,
                    7,
                )),
                Event::CameraSelectionWait {
                    previous: camera_snapshot(1, 2, 3),
                    expected: CameraSelectionExpectation {
                        car_index: Some(12),
                        group: 6,
                        camera: 7,
                    },
                    timeout: Duration::from_millis(25),
                },
            ]
        );
    }

    #[tokio::test]
    async fn camera_set_state_snapshots_sends_then_waits() {
        let Fixture { events, use_cases } = fixture();

        let result = use_cases
            .camera_set_state(CameraState::UI_HIDDEN)
            .await
            .expect("camera state should succeed");

        assert_eq!(result, camera_state(CameraState::UI_HIDDEN.bits()));
        assert_eq!(
            *events.lock().expect("events mutex poisoned"),
            vec![
                Event::CameraStateSnapshot,
                Event::Send(BroadcastCommand::CameraSetState(CameraState::UI_HIDDEN)),
                Event::CameraStateWait {
                    previous: camera_state(0),
                    expected: CameraStateExpectation {
                        state: CameraState::UI_HIDDEN.bits(),
                    },
                    timeout: Duration::from_millis(25),
                },
            ]
        );
    }

    #[tokio::test]
    async fn replay_set_play_speed_snapshots_sends_then_waits() {
        let Fixture { events, use_cases } = fixture();

        let result = use_cases
            .replay_set_play_speed(2, true)
            .await
            .expect("replay speed should succeed");

        assert_eq!(result, replay_speed(2, true));
        assert_eq!(
            *events.lock().expect("events mutex poisoned"),
            vec![
                Event::ReplaySpeedSnapshot,
                Event::Send(BroadcastCommand::ReplaySetPlaySpeed(2, true)),
                Event::ReplaySpeedWait {
                    previous: replay_speed(0, false),
                    expected: ReplaySpeedExpectation {
                        speed: 2,
                        is_slow_motion: true,
                    },
                    timeout: Duration::from_millis(25),
                },
            ]
        );
    }

    #[tokio::test]
    async fn replay_position_use_cases_wait_for_position_change() {
        let Fixture { events, use_cases } = fixture();

        let result = use_cases
            .replay_set_play_position(ReplayPositionMode::Current, 20)
            .await
            .expect("replay position should succeed");

        assert_eq!(result, replay_position(20, 1, 3.0));
        assert_eq!(
            *events.lock().expect("events mutex poisoned"),
            vec![
                Event::ReplayPositionSnapshot,
                Event::Send(BroadcastCommand::ReplaySetPlayPosition(
                    ReplayPositionMode::Current,
                    20,
                )),
                Event::ReplayPositionWait {
                    previous: replay_position(10, 1, 2.0),
                    expected: ReplayPositionExpectation::AnyChange,
                    timeout: Duration::from_millis(25),
                },
            ]
        );
    }

    #[tokio::test]
    async fn pit_command_stream_sends_all_commands_then_waits() {
        let Fixture { events, use_cases } = fixture();

        let result = use_cases
            .pit_command_stream(vec![PitCommand::Clear, PitCommand::Fuel(5)])
            .await
            .expect("pit stream should succeed");

        assert_eq!(result, pit_snapshot(1));
        assert_eq!(
            *events.lock().expect("events mutex poisoned"),
            vec![
                Event::PitSnapshot,
                Event::Send(BroadcastCommand::PitCommand(PitCommand::Clear)),
                Event::Send(BroadcastCommand::PitCommand(PitCommand::Fuel(5))),
                Event::PitWait {
                    previous: pit_snapshot(0),
                    expected: PitServiceExpectation::AnyChange,
                    timeout: Duration::from_millis(25),
                },
            ]
        );
    }

    #[tokio::test]
    async fn telemetry_and_force_feedback_wait_for_expected_state() {
        let Fixture { events, use_cases } = fixture();

        let telemetry = use_cases
            .telemetry_command(TelemetryCommandMode::Stop)
            .await
            .expect("telemetry command should succeed");
        assert_eq!(telemetry, telemetry_snapshot(false, false));

        let force_feedback = use_cases
            .force_feedback_command(20.0)
            .await
            .expect("force feedback command should succeed");
        assert_eq!(force_feedback, ForceFeedbackSnapshot { max_force: 20.0 });

        assert_eq!(
            *events.lock().expect("events mutex poisoned"),
            vec![
                Event::TelemetrySnapshot,
                Event::Send(BroadcastCommand::TelemetryCommand(
                    TelemetryCommandMode::Stop,
                )),
                Event::TelemetryWait {
                    previous: telemetry_snapshot(true, true),
                    expected: TelemetryLoggingExpectation {
                        is_disk_logging_enabled: false,
                    },
                    timeout: Duration::from_millis(25),
                },
                Event::ForceFeedbackSnapshot,
                Event::Send(BroadcastCommand::FFBCommand(20.0)),
                Event::ForceFeedbackWait {
                    previous: ForceFeedbackSnapshot { max_force: 10.0 },
                    expected: ForceFeedbackExpectation { max_force: 20.0 },
                    timeout: Duration::from_millis(25),
                },
            ]
        );
    }

    #[tokio::test]
    async fn disabled_observation_fails_observed_use_case_but_ack_still_sends() {
        let events = Arc::new(StdMutex::new(Vec::new()));
        let commands = Arc::new(FakeCommands {
            events: Arc::clone(&events),
            error: StdMutex::new(None),
        });
        let disabled = Arc::new(crate::broadcast_app::DisabledObservationPort);
        let use_cases = BroadcastUseCases::new(
            commands,
            disabled.clone(),
            disabled.clone(),
            disabled.clone(),
            disabled.clone(),
            disabled,
            Duration::from_millis(25),
        );

        let error = use_cases
            .camera_switch_position(1, 2, 3)
            .await
            .expect_err("camera switch should require observation");
        assert!(matches!(error, BroadcastError::ObservationDisabled));

        use_cases
            .reload_textures(None)
            .await
            .expect("ack command should succeed");

        assert_eq!(
            *events.lock().expect("events mutex poisoned"),
            vec![Event::Send(BroadcastCommand::ReloadAllTextures)]
        );
    }

    #[tokio::test]
    async fn ack_use_cases_emit_expected_commands() {
        let Fixture { events, use_cases } = fixture();

        use_cases
            .chat_command(BroadcastCommand::ChatCommand(ChatCommandMode::Reply))
            .await
            .expect("chat command should send");
        use_cases
            .replay_search_session_time(2, 3)
            .await
            .expect("session time search should send");

        assert_eq!(
            *events.lock().expect("events mutex poisoned"),
            vec![
                Event::Send(BroadcastCommand::ChatCommand(ChatCommandMode::Reply)),
                Event::Send(BroadcastCommand::ReplaySearchSessionTime(2, 3)),
            ]
        );
    }
}
