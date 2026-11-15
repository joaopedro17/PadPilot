//! Virtual Xbox 360 controller through the ViGEmBus driver.

use std::time::{Duration, Instant};

use padpilot_core::gamepad::GamepadState;
use tracing::{debug, info};
use vigem_client::{Client, TargetId, XGamepad, Xbox360Wired};

use super::{Error, VirtualController};

/// The driver drops a report (`ERROR_NO_MORE_ITEMS`) while Windows is not yet
/// polling the controller. Measured: only the first report right after
/// plug-in, ready within 50 ms. A dropped report is sent again for this long.
const DELIVERY_TIMEOUT: Duration = Duration::from_millis(500);
const ERROR_NO_MORE_ITEMS: u32 = 259;

pub struct Vigem {
    target: Xbox360Wired<Client>,
}

impl Vigem {
    pub fn connect() -> Result<Self, Error> {
        let client =
            Client::connect().map_err(|err| failed("ViGEmBus driver not reachable", err))?;
        let mut target = Xbox360Wired::new(client, TargetId::XBOX360_WIRED);
        target
            .plugin()
            .map_err(|err| failed("cannot plug in the virtual controller", err))?;
        target
            .wait_ready()
            .map_err(|err| failed("virtual controller never became ready", err))?;
        info!(target: "controller", "virtual Xbox 360 controller connected");
        Ok(Self { target })
    }
}

impl VirtualController for Vigem {
    fn send(&mut self, state: GamepadState) -> Result<(), Error> {
        let gamepad = XGamepad {
            thumb_lx: state.left_stick_x,
            right_trigger: state.right_trigger,
            left_trigger: state.left_trigger,
            ..XGamepad::default()
        };
        let deadline = Instant::now() + DELIVERY_TIMEOUT;
        loop {
            match self.target.update(&gamepad) {
                Err(vigem_client::Error::WinError(ERROR_NO_MORE_ITEMS))
                    if Instant::now() < deadline =>
                {
                    debug!(target: "controller", "report dropped by the driver, sending again");
                    std::thread::sleep(Duration::from_millis(1));
                }
                result => {
                    return result
                        .map_err(|err| failed("cannot update the virtual controller", err));
                }
            }
        }
    }
}

impl Drop for Vigem {
    fn drop(&mut self) {
        info!(target: "controller", "virtual Xbox 360 controller disconnected");
    }
}

fn failed(what: &str, err: vigem_client::Error) -> Error {
    Error(format!("{what}: {err:?}"))
}
