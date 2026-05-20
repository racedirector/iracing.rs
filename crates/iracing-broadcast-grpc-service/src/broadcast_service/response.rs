use crate::broadcast::PitCommandResponse;

pub(crate) fn empty_pit_command_response() -> PitCommandResponse {
    PitCommandResponse {
        service_flags: 0,
        fuel: 0.0,
        lf_pressure: 0.0,
        rf_pressure: 0.0,
        lr_pressure: 0.0,
        rr_pressure: 0.0,
        tire_compound: 0,
    }
}
