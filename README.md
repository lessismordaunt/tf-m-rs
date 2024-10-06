# Unofficial Rust bindings to Trusted Firmware-M

These bindings are currently only intended for the development of standalone NS applications which may call into the Secure World via PSA service functions.
Building TF-M proper is handled by the `tf-m-rs-sys` crate, which also provides low-level bindings to the exported NS API.

Embedded HAL/BSP crates will need to be sought independently for your platform of choice.

Stitching of the final flash image is largely platform-dependent and is not considered by this project. However, conversion from ELF to binary form is performed.

### Disclaimer

This project is completely unofficial and is not developed in accordance with software security practices. There are no guarantees or warranties provided.

> [!CAUTION]
> Do not use this in production!
