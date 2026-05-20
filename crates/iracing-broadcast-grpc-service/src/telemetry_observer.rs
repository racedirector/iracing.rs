use std::{error::Error as StdError, fmt, sync::Arc, time::Duration};

use iracing_sdk::{FrameAdapter, IRacingSDKError, Provider, VariableSchema};
use tokio::sync::Mutex;

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum TelemetryObserverError {
    Sdk(IRacingSDKError),
    Timeout,
    EndOfSource,
}

impl fmt::Display for TelemetryObserverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sdk(error) => write!(f, "{error}"),
            Self::Timeout => write!(f, "telemetry observation timed out"),
            Self::EndOfSource => write!(f, "telemetry source ended"),
        }
    }
}

impl StdError for TelemetryObserverError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Sdk(error) => Some(error),
            Self::Timeout | Self::EndOfSource => None,
        }
    }
}

impl From<IRacingSDKError> for TelemetryObserverError {
    fn from(value: IRacingSDKError) -> Self {
        Self::Sdk(value)
    }
}

#[allow(dead_code)]
pub(crate) struct TelemetryObserver<P> {
    provider: Arc<Mutex<P>>,
    schema: Arc<VariableSchema>,
}

impl<P> Clone for TelemetryObserver<P> {
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            schema: Arc::clone(&self.schema),
        }
    }
}

#[allow(dead_code)]
impl<P> TelemetryObserver<P>
where
    P: Provider,
{
    pub(crate) fn new(provider: Arc<Mutex<P>>, schema: Arc<VariableSchema>) -> Self {
        Self { provider, schema }
    }

    pub(crate) fn validate<A>(&self) -> Result<(), TelemetryObserverError>
    where
        A: FrameAdapter,
    {
        A::validate_schema(&self.schema)?;
        Ok(())
    }

    pub(crate) async fn snapshot<A>(&self) -> Result<A, TelemetryObserverError>
    where
        A: FrameAdapter,
    {
        let validation = A::validate_schema(&self.schema)?;
        let mut provider = self.provider.lock().await;
        let packet = provider
            .next_frame()
            .await?
            .ok_or(TelemetryObserverError::EndOfSource)?;

        Ok(A::adapt(&packet, &validation))
    }

    pub(crate) async fn wait_for_change_matching<A>(
        &self,
        previous: A,
        timeout: Duration,
        matches: impl Fn(&A) -> bool,
    ) -> Result<A, TelemetryObserverError>
    where
        A: FrameAdapter + PartialEq,
    {
        let validation = A::validate_schema(&self.schema)?;
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Err(TelemetryObserverError::Timeout);
            }

            let remaining = deadline.saturating_duration_since(now);
            let next_frame = {
                let mut provider = self.provider.lock().await;
                tokio::time::timeout(remaining, provider.next_frame()).await
            };

            let packet = match next_frame {
                Ok(Ok(Some(packet))) => packet,
                Ok(Ok(None)) => return Err(TelemetryObserverError::EndOfSource),
                Ok(Err(error)) => return Err(TelemetryObserverError::Sdk(error)),
                Err(_) => return Err(TelemetryObserverError::Timeout),
            };

            let current = A::adapt(&packet, &validation);
            if current != previous && matches(&current) {
                return Ok(current);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use std::{
        collections::{HashMap, VecDeque},
        sync::Arc,
        time::Duration,
    };

    use async_trait::async_trait;
    use iracing_sdk::{
        FramePacket, IRacingSDKError, Provider, VariableInfo, VariableSchema, VariableType,
    };
    use tokio::sync::Mutex;

    use super::{TelemetryObserver, TelemetryObserverError};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, iracing_sdk::IRacingTelemetryFrame)]
    pub(crate) struct CameraSelectionTelemetry {
        #[field_name = "CamCarIdx"]
        #[fail_if_missing]
        pub car_index: i32,

        #[field_name = "CamGroupNumber"]
        #[fail_if_missing]
        pub group: i32,

        #[field_name = "CamCameraNumber"]
        #[fail_if_missing]
        pub camera: i32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, iracing_sdk::IRacingTelemetryFrame)]
    pub(crate) struct ReplaySpeedTelemetry {
        #[field_name = "ReplayPlaySpeed"]
        #[fail_if_missing]
        pub speed: i32,

        #[field_name = "ReplayPlaySlowMotion"]
        #[fail_if_missing]
        pub is_slow_motion: bool,
    }

    struct BroadcastRpcHarness<P> {
        telemetry: TelemetryObserver<P>,
    }

    impl<P> BroadcastRpcHarness<P>
    where
        P: Provider,
    {
        async fn camera_snapshot(
            &self,
        ) -> Result<CameraSelectionTelemetry, TelemetryObserverError> {
            self.telemetry.snapshot().await
        }

        async fn replay_speed_snapshot(
            &self,
        ) -> Result<ReplaySpeedTelemetry, TelemetryObserverError> {
            self.telemetry.snapshot().await
        }
    }

    enum PlannedFrame {
        Ready(Result<Option<FramePacket>, IRacingSDKError>),
        Delayed {
            delay: Duration,
            result: Result<Option<FramePacket>, IRacingSDKError>,
        },
    }

    struct FakeProvider {
        schema: Arc<VariableSchema>,
        frames: VecDeque<PlannedFrame>,
    }

    impl FakeProvider {
        fn new(
            schema: Arc<VariableSchema>,
            frames: impl IntoIterator<Item = PlannedFrame>,
        ) -> Self {
            Self {
                schema,
                frames: frames.into_iter().collect(),
            }
        }
    }

    #[async_trait(?Send)]
    impl Provider for FakeProvider {
        async fn next_frame(&mut self) -> iracing_sdk::Result<Option<FramePacket>> {
            match self
                .frames
                .pop_front()
                .unwrap_or(PlannedFrame::Ready(Ok(None)))
            {
                PlannedFrame::Ready(result) => result,
                PlannedFrame::Delayed { delay, result } => {
                    tokio::time::sleep(delay).await;
                    result
                }
            }
        }

        async fn session_yaml(&mut self, _version: u32) -> iracing_sdk::Result<Option<String>> {
            Ok(None)
        }

        fn tick_rate(&self) -> f64 {
            let _ = &self.schema;
            60.0
        }
    }

    fn make_variable_info(name: &str, data_type: VariableType, offset: usize) -> VariableInfo {
        VariableInfo {
            name: name.to_string(),
            data_type,
            offset,
            count: 1,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        }
    }

    fn make_schema(
        entries: &[(&str, VariableType, usize)],
        frame_size: usize,
    ) -> Arc<VariableSchema> {
        let variables = entries
            .iter()
            .map(|(name, data_type, offset)| {
                (
                    (*name).to_string(),
                    make_variable_info(name, *data_type, *offset),
                )
            })
            .collect::<HashMap<_, _>>();

        Arc::new(VariableSchema::new(variables, frame_size).expect("schema should be valid"))
    }

    fn full_schema() -> Arc<VariableSchema> {
        make_schema(
            &[
                ("CamCarIdx", VariableType::Int32, 0),
                ("CamGroupNumber", VariableType::Int32, 4),
                ("CamCameraNumber", VariableType::Int32, 8),
                ("ReplayPlaySpeed", VariableType::Int32, 12),
                ("ReplayPlaySlowMotion", VariableType::Bool, 16),
            ],
            17,
        )
    }

    fn schema_missing_camera_fields() -> Arc<VariableSchema> {
        make_schema(
            &[
                ("ReplayPlaySpeed", VariableType::Int32, 12),
                ("ReplayPlaySlowMotion", VariableType::Bool, 16),
            ],
            17,
        )
    }

    fn make_packet(
        schema: Arc<VariableSchema>,
        car_index: i32,
        group: i32,
        camera: i32,
        replay_speed: i32,
        is_slow_motion: bool,
    ) -> FramePacket {
        let mut data = vec![0u8; 17];
        data[0..4].copy_from_slice(&car_index.to_le_bytes());
        data[4..8].copy_from_slice(&group.to_le_bytes());
        data[8..12].copy_from_slice(&camera.to_le_bytes());
        data[12..16].copy_from_slice(&replay_speed.to_le_bytes());
        data[16] = u8::from(is_slow_motion);
        FramePacket::new(data, 7, 11, schema)
    }

    fn make_camera_packet(
        schema: Arc<VariableSchema>,
        car_index: i32,
        group: i32,
        camera: i32,
    ) -> FramePacket {
        make_packet(schema, car_index, group, camera, 0, false)
    }

    fn make_replay_packet(
        schema: Arc<VariableSchema>,
        speed: i32,
        is_slow_motion: bool,
    ) -> FramePacket {
        make_packet(schema, 0, 0, 0, speed, is_slow_motion)
    }

    #[test]
    fn constructor_validates_adapter_schema_once() {
        let schema = full_schema();
        let observer = TelemetryObserver::new(
            Arc::new(Mutex::new(FakeProvider::new(Arc::clone(&schema), []))),
            Arc::clone(&schema),
        );

        observer
            .validate::<CameraSelectionTelemetry>()
            .expect("camera schema should validate");
        observer
            .validate::<ReplaySpeedTelemetry>()
            .expect("replay schema should validate");

        let missing_schema = schema_missing_camera_fields();
        let missing_observer = TelemetryObserver::new(
            Arc::new(Mutex::new(FakeProvider::new(
                Arc::clone(&missing_schema),
                [],
            ))),
            missing_schema,
        );

        match missing_observer.validate::<CameraSelectionTelemetry>() {
            Err(TelemetryObserverError::Sdk(IRacingSDKError::Parse { context, details })) => {
                assert_eq!(context, "Frame adapter validation");
                assert!(details.contains("CamCarIdx"));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn snapshot_adapts_first_frame() {
        let schema = full_schema();
        let expected = CameraSelectionTelemetry {
            car_index: 42,
            group: 3,
            camera: 7,
        };
        let observer = TelemetryObserver::new(
            Arc::new(Mutex::new(FakeProvider::new(
                Arc::clone(&schema),
                [PlannedFrame::Ready(Ok(Some(make_camera_packet(
                    Arc::clone(&schema),
                    expected.car_index,
                    expected.group,
                    expected.camera,
                ))))],
            ))),
            schema,
        );

        let actual = observer
            .snapshot::<CameraSelectionTelemetry>()
            .await
            .expect("snapshot should succeed");

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn snapshot_end_of_source() {
        let schema = full_schema();
        let observer = TelemetryObserver::new(
            Arc::new(Mutex::new(FakeProvider::new(
                Arc::clone(&schema),
                [PlannedFrame::Ready(Ok(None))],
            ))),
            schema,
        );

        match observer.snapshot::<CameraSelectionTelemetry>().await {
            Err(TelemetryObserverError::EndOfSource) => {}
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn wait_returns_changed_matching_frame() {
        let schema = full_schema();
        let previous = CameraSelectionTelemetry {
            car_index: 1,
            group: 2,
            camera: 3,
        };
        let matching = CameraSelectionTelemetry {
            car_index: 9,
            group: 4,
            camera: 5,
        };
        let observer = TelemetryObserver::new(
            Arc::new(Mutex::new(FakeProvider::new(
                Arc::clone(&schema),
                [
                    PlannedFrame::Ready(Ok(Some(make_camera_packet(
                        Arc::clone(&schema),
                        previous.car_index,
                        previous.group,
                        previous.camera,
                    )))),
                    PlannedFrame::Ready(Ok(Some(make_camera_packet(
                        Arc::clone(&schema),
                        matching.car_index,
                        matching.group,
                        matching.camera,
                    )))),
                ],
            ))),
            schema,
        );

        let actual = observer
            .wait_for_change_matching(previous, Duration::from_millis(100), |current| {
                current.group == matching.group
            })
            .await
            .expect("wait should find a changed frame");

        assert_eq!(actual, matching);
    }

    #[tokio::test]
    async fn wait_times_out_without_match() {
        let schema = full_schema();
        let previous = CameraSelectionTelemetry {
            car_index: 1,
            group: 2,
            camera: 3,
        };
        let observer = TelemetryObserver::new(
            Arc::new(Mutex::new(FakeProvider::new(
                Arc::clone(&schema),
                [PlannedFrame::Delayed {
                    delay: Duration::from_millis(50),
                    result: Ok(Some(make_camera_packet(
                        Arc::clone(&schema),
                        previous.car_index,
                        previous.group,
                        previous.camera,
                    ))),
                }],
            ))),
            schema,
        );

        match observer
            .wait_for_change_matching(previous, Duration::from_millis(10), |_| true)
            .await
        {
            Err(TelemetryObserverError::Timeout) => {}
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn wait_returns_end_of_source() {
        let schema = full_schema();
        let previous = CameraSelectionTelemetry {
            car_index: 1,
            group: 2,
            camera: 3,
        };
        let observer = TelemetryObserver::new(
            Arc::new(Mutex::new(FakeProvider::new(
                Arc::clone(&schema),
                [PlannedFrame::Ready(Ok(None))],
            ))),
            schema,
        );

        match observer
            .wait_for_change_matching(previous, Duration::from_millis(20), |_| true)
            .await
        {
            Err(TelemetryObserverError::EndOfSource) => {}
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn provider_error_is_preserved() {
        let schema = full_schema();
        let expected = IRacingSDKError::connection_failed("provider blew up");
        let observer = TelemetryObserver::new(
            Arc::new(Mutex::new(FakeProvider::new(
                Arc::clone(&schema),
                [PlannedFrame::Ready(Err(expected))],
            ))),
            schema,
        );

        match observer.snapshot::<CameraSelectionTelemetry>().await {
            Err(TelemetryObserverError::Sdk(IRacingSDKError::Connection { reason, .. })) => {
                assert_eq!(reason, "provider blew up");
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn shared_provider_serializes_reads() {
        let schema = full_schema();
        let first = CameraSelectionTelemetry {
            car_index: 10,
            group: 20,
            camera: 30,
        };
        let second = CameraSelectionTelemetry {
            car_index: 11,
            group: 21,
            camera: 31,
        };
        let provider = Arc::new(Mutex::new(FakeProvider::new(
            Arc::clone(&schema),
            [
                PlannedFrame::Ready(Ok(Some(make_camera_packet(
                    Arc::clone(&schema),
                    first.car_index,
                    first.group,
                    first.camera,
                )))),
                PlannedFrame::Ready(Ok(Some(make_camera_packet(
                    Arc::clone(&schema),
                    second.car_index,
                    second.group,
                    second.camera,
                )))),
            ],
        )));
        let observer = TelemetryObserver::new(Arc::clone(&provider), schema);
        let cloned = observer.clone();

        let (left, right) = tokio::join!(
            observer.snapshot::<CameraSelectionTelemetry>(),
            cloned.snapshot::<CameraSelectionTelemetry>()
        );

        let left = left.expect("left snapshot should succeed");
        let right = right.expect("right snapshot should succeed");

        assert_ne!(left, right);
        assert!(
            matches!((left, right), (a, b) if (a == first && b == second) || (a == second && b == first))
        );
    }

    #[tokio::test]
    async fn same_observer_reads_different_telemetry_shapes_over_its_lifetime() {
        let schema = full_schema();
        let observer = TelemetryObserver::new(
            Arc::new(Mutex::new(FakeProvider::new(
                Arc::clone(&schema),
                [
                    PlannedFrame::Ready(Ok(Some(make_camera_packet(
                        Arc::clone(&schema),
                        44,
                        5,
                        6,
                    )))),
                    PlannedFrame::Ready(Ok(Some(make_replay_packet(Arc::clone(&schema), 2, true)))),
                ],
            ))),
            schema,
        );

        let camera = observer
            .snapshot::<CameraSelectionTelemetry>()
            .await
            .expect("camera snapshot should succeed");
        let replay = observer
            .snapshot::<ReplaySpeedTelemetry>()
            .await
            .expect("replay snapshot should succeed");

        assert_eq!(
            camera,
            CameraSelectionTelemetry {
                car_index: 44,
                group: 5,
                camera: 6,
            }
        );
        assert_eq!(
            replay,
            ReplaySpeedTelemetry {
                speed: 2,
                is_slow_motion: true,
            }
        );
    }

    #[tokio::test]
    async fn observer_can_be_a_struct_field_and_called_from_multiple_places() {
        let schema = full_schema();
        let harness = BroadcastRpcHarness {
            telemetry: TelemetryObserver::new(
                Arc::new(Mutex::new(FakeProvider::new(
                    Arc::clone(&schema),
                    [
                        PlannedFrame::Ready(Ok(Some(make_camera_packet(
                            Arc::clone(&schema),
                            7,
                            8,
                            9,
                        )))),
                        PlannedFrame::Ready(Ok(Some(make_replay_packet(
                            Arc::clone(&schema),
                            3,
                            false,
                        )))),
                    ],
                ))),
                schema,
            ),
        };

        let camera = harness
            .camera_snapshot()
            .await
            .expect("camera snapshot should succeed");
        let replay = harness
            .replay_speed_snapshot()
            .await
            .expect("replay snapshot should succeed");

        assert_eq!(
            camera,
            CameraSelectionTelemetry {
                car_index: 7,
                group: 8,
                camera: 9,
            }
        );
        assert_eq!(
            replay,
            ReplaySpeedTelemetry {
                speed: 3,
                is_slow_motion: false,
            }
        );
    }
}
