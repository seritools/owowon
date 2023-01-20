use crate::data::units::ProbeAttenuation;

pub const PROBE_ATTENUATIONS: [ProbeAttenuation; 5] = [
    ProbeAttenuation(1),
    ProbeAttenuation(10),
    ProbeAttenuation(100),
    ProbeAttenuation(1000),
    ProbeAttenuation(10000),
];
