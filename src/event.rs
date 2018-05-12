//! Vendor-specific events for BlueNRG controllers.
//!
//! The BlueNRG implementation defines several additional events that are packaged as
//! vendor-specific events by the Bluetooth HCI. This module defines those events and functions to
//! deserialize buffers into them.
extern crate bluetooth_hci as hci;

use byteorder::{ByteOrder, LittleEndian};
use core::cmp::{min, PartialEq};
use core::convert::{TryFrom, TryInto};
use core::fmt::{Debug, Formatter, Result as FmtResult};
use core::mem;

/// Enumeration of potential errors when deserializing events.
#[derive(Clone, Copy, Debug)]
pub enum Error {
    /// The event is not recoginized. Includes the unknown opcode.
    UnknownEvent(u16),

    /// For the HalInitialized event: the reset reason was not recognized. Includes the unrecognized
    /// byte.
    UnknownResetReason(u8),

    /// For the EventsLost event: The event included unrecognized event flags. Includes the entire
    /// bitfield.
    BadEventFlags(u64),

    /// For the CrashReport event: The crash reason was not recognized. Includes the unrecognized
    /// byte.
    UnknownCrashReason(u8),

    /// For the GAP Pairing Complete event: The status was not recognized. Includes the unrecognized
    /// byte.
    BadGapPairingStatus(u8),

    /// For the GAP Device Found event: the type of event was not recognized. Includes the
    /// unrecognized byte.
    BadGapDeviceFoundEvent(u8),

    /// For the GAP Device Found event: the type of BDADDR was not recognized. Includes the
    /// unrecognized byte.
    BadGapBdAddrType(u8),

    /// For the GAP Procedure Complete event: The procedure code was not recognized. Includes the
    /// unrecognized byte.
    BadGapProcedure(u8),

    /// For the GAP Procedure Complete event: The procedure status was not recognized. Includes the
    /// unrecognized byte.
    BadGapProcedureStatus(u8),

    /// For the GAP Device Found event: the RSSI code at the end of the packet indicated that the
    /// RSSI is unavailable.
    GapRssiUnavailable,

    /// For any L2CAP event: The event data length did not match the expected length. The first
    /// field is the required length, and the second is the actual length.
    BadL2CapDataLength(u8, u8),

    /// For any L2CAP event: The L2CAP length did not match the expected length. The first field is
    /// the required length, and the second is the actual length.
    BadL2CapLength(u16, u16),

    /// For any L2CAP response event: The L2CAP command was rejected, but the rejection reason was
    /// not recognized. Includes the unknown value.
    BadL2CapRejectionReason(u16),

    /// For the L2CapConnectionUpdateResponse event: The code byte did not indicate either Rejected
    /// or Updated. Includes the invalid byte.
    BadL2CapConnectionResponseCode(u8),

    /// For the L2CapConnectionUpdateResponse event: The command was accepted, but the result was
    /// not recognized. It did not indicate the parameters were either updated or rejected. Includes
    /// the unknown value.
    BadL2CapConnectionResponseResult(u16),

    /// For the L2CapconnectionUpdateRequest event: The provided interval is invalid. Potential
    /// errors:
    ///
    /// - Either the minimum or maximum is out of range. The minimum value for either is 6, and the
    ///   maximum is 3200.
    ///
    /// - The min is greater than the max
    ///
    /// See the Bluetooth specification, Vol 3, Part A, Section 4.20. Versions 4.1, 4.2 and 5.0.
    ///
    /// Inclues the provided minimum and maximum, respectively.
    BadL2CapConnectionUpdateRequestInterval(u16, u16),

    /// For the L2CapconnectionUpdateRequest event: The provided slave latency is invalid. The
    /// maximum value for slave latency is defined in terms of the timeout and maximum connection
    /// interval.
    ///
    /// - connIntervalMax = Interval Max * 1.25 ms
    /// - connSupervisionTimeout = Timeout Multiplier * 10 ms
    /// - maxSlaveLatency = min(500, ((connSupervisionTimeout / (2 * connIntervalMax)) - 1))
    ///
    /// See the Bluetooth specification, Vol 3, Part A, Section 4.20. Versions 4.1, 4.2 and 5.0.
    ///
    /// Inclues the provided value and maximum allowed value, respectively.
    BadL2CapConnectionUpdateRequestLatency(u16, u16),

    /// For the L2CapconnectionUpdateRequest event: The provided timeout multiplier is invalid. The
    /// timeout multiplier field shall have a value in the range of 10 to 3200 (inclusive).
    ///
    /// See the Bluetooth specification, Vol 3, Part A, Section 4.20. Versions 4.1, 4.2 and 5.0.
    ///
    /// Inclues the provided value.
    BadL2CapConnectionUpdateRequestTimeoutMult(u16),

    /// For the ATT Find Information Response event: The format code is invalid. Includes the
    /// unrecognized byte.
    BadAttFindInformationResponseFormat(u8),

    /// For the ATT Find Information Response event: The format code indicated 16-bit UUIDs, but
    /// the packet ends with a partial pair.
    AttFindInformationResponsePartialPair16,

    /// For the ATT Find Information Response event: The format code indicated 128-bit UUIDs, but
    /// the packet ends with a partial pair.
    AttFindInformationResponsePartialPair128,

    /// For the ATT Find by Type Value Response event: The packet ends with a partial attribute
    /// pair.
    AttFindByTypeValuePartial,

    /// For the ATT Read by Type Response event: The packet ends with a partial attribute
    /// handle-value pair.
    AttReadByTypeResponsePartial,

    /// For the ATT Read by Group Type Response event: The packet ends with a partial attribute
    /// data group.
    AttReadByGroupTypeResponsePartial,

    /// For the GATT Procedure Complete event: The status code was not recognized. Includes the
    /// unrecognized byte.
    BadGattProcedureStatus(u8),

    /// For the ATT Error Response event: The request opcode was not recognized. Includes the
    /// unrecognized byte.
    BadAttRequestOpcode(u8),
}

/// Vendor-specific events for the BlueNRG-MS controllers.
#[derive(Clone, Copy, Debug)]
pub enum BlueNRGEvent {
    /// When the BlueNRG-MS firmware is started normally, it gives a Evt_Blue_Initialized event to
    /// the user to indicate the system has started.
    HalInitialized(ResetReason),

    /// If the host fails to read events from the controller quickly enough, the controller will
    /// generate an EventsLost event. This event is never lost; it is inserted as soon as space is
    /// available in the Tx queue.
    #[cfg(feature = "ms")]
    EventsLost(EventFlags),

    /// The fault data event is automatically sent after the HalInitialized event in case of NMI or
    /// Hard fault (ResetReason::Crash).
    #[cfg(feature = "ms")]
    CrashReport(FaultData),

    /// This event is generated by the controller when the limited discoverable mode ends due to
    /// timeout (180 seconds). No parameters in the event.
    GapLimitedDiscoverable,

    /// This event is generated when the pairing process has completed successfully or a pairing
    /// procedure timeout has occurred or the pairing has failed.  This is to notify the application
    /// that we have paired with a remote device so that it can take further actions or to notify
    /// that a timeout has occurred so that the upper layer can decide to disconnect the link.
    GapPairingComplete(GapPairingComplete),

    /// This event is generated by the Security manager to the application when a pass key is
    /// required for pairing.  When this event is received, the application has to respond with the
    /// aci_gap_pass_key_response() command.
    GapPassKeyRequest(ConnectionHandle),

    /// This event is generated by the Security manager to the application when the application has
    /// set that authorization is required for reading/writing of attributes. This event will be
    /// generated as soon as the pairing is complete. When this event is received,
    /// aci_gap_authorization_response() command should be used by the application.
    GapAuthorizationRequest(ConnectionHandle),

    /// This event is generated when the slave security request is successfully sent to the master.
    GapSlaveSecurityInitiated,

    /// This event is generated on the slave when a aci_gap_slave_security_request() is called to
    /// reestablish the bond with a master but the master has lost the bond. When this event is
    /// received, the upper layer has to issue the command aci_gap_allow_rebond() in order to allow
    /// the slave to continue the pairing process with the master. On the master this event is
    /// raised when aci_gap_send_pairing_request() is called to reestablish a bond with a slave but
    /// the slave has lost the bond. In order to create a new bond the master has to launch
    /// aci_gap_send_pairing_request() with force_rebond set to 1.
    GapBondLost,

    /// The event is given by the GAP layer to the upper layers when a device is discovered during
    /// scanning as a consequence of one of the GAP procedures started by the upper layers.
    GapDeviceFound(GapDeviceFound),

    /// This event is sent by the GAP to the upper layers when a procedure previously started has
    /// been terminated by the upper layer or has completed for any other reason
    GapProcedureComplete(GapProcedureComplete),

    /// This event is sent only by a privacy enabled Peripheral. The event is sent to the upper
    /// layers when the peripheral is unsuccessful in resolving the resolvable address of the peer
    /// device after connecting to it.
    #[cfg(feature = "ms")]
    GapAddressNotResolved(ConnectionHandle),

    /// This event is generated when the reconnection address is generated during the general
    /// connection establishment procedure. The same address is set to the peer device also as a
    /// part of the general connection establishment procedure. In order to make use of the
    /// reconnection address the next time while connecting to the bonded peripheral, the
    /// application needs to set its own address as well as the peer address to which it wants to
    /// connect to this reconnection address.
    #[cfg(not(feature = "ms"))]
    GapReconnectionAddress(BdAddrBuffer),

    /// This event is generated when the master responds to the L2CAP connection update request
    /// packet. For more info see CONNECTION PARAMETER UPDATE RESPONSE and COMMAND REJECT in
    /// Bluetooth Core v4.0 spec.
    L2CapConnectionUpdateResponse(L2CapConnectionUpdateResponse),

    /// This event is generated when the master does not respond to the connection update request
    /// within 30 seconds.
    L2CapProcedureTimeout(ConnectionHandle),

    /// The event is given by the L2CAP layer when a connection update request is received from the
    /// slave.  The application has to respond by calling
    /// aci_l2cap_connection_parameter_update_response().
    L2CapConnectionUpdateRequest(L2CapConnectionUpdateRequest),

    /// This event is generated to the application by the ATT server when a client modifies any
    /// attribute on the server, as consequence of one of the following ATT procedures:
    /// - write without response
    /// - signed write without response
    /// - write characteristic value
    /// - write long characteristic value
    /// - reliable write
    GattAttributeModified(GattAttributeModified),

    /// This event is generated when a ATT client procedure completes either with error or
    /// successfully.
    GattProcedureTimeout(ConnectionHandle),

    /// This event is generated in response to an Exchange MTU request.
    AttExchangeMtuResponse(AttExchangeMtuResponse),

    /// This event is generated in response to a Find Information Request. See Find Information
    /// Response in Bluetooth Core v4.0 spec.
    AttFindInformationResponse(AttFindInformationResponse),

    /// This event is generated in response to a Find By Type Value Request.
    AttFindByTypeValueResponse(AttFindByTypeValueResponse),

    /// This event is generated in response to a Read by Type Request.
    AttReadByTypeResponse(AttReadByTypeResponse),

    /// This event is generated in response to a Read Request.
    AttReadResponse(AttReadResponse),

    /// This event is generated in response to a Read Blob Request. The value in the response is the
    /// partial value starting from the offset in the request. See the Bluetooth Core v4.1 spec, Vol
    /// 3, section 3.4.4.5 and 3.4.4.6.
    AttReadBlobResponse(AttReadResponse),

    /// This event is generated in response to a Read Multiple Request. The value in the response is
    /// the set of values requested from the request. See the Bluetooth Core v4.1 spec, Vol 3,
    /// section 3.4.4.7 and 3.4.4.8.
    AttReadMultipleResponse(AttReadResponse),

    /// This event is generated in response to a Read By Group Type Request. See the Bluetooth Core
    /// v4.1 spec, Vol 3, section 3.4.4.9 and 3.4.4.10.
    AttReadByGroupTypeResponse(AttReadByGroupTypeResponse),

    /// This event is generated in response to a Prepare Write Request. See the Bluetooth Core v4.1
    /// spec, Vol 3, Part F, section 3.4.6.1 and 3.4.6.2
    AttPrepareWriteResponse(AttPrepareWriteResponse),

    /// This event is generated in response to an Execute Write Request. See the Bluetooth Core v4.1
    /// spec, Vol 3, Part F, section 3.4.6.3 and 3.4.6.4
    AttExecuteWriteResponse(ConnectionHandle),

    /// This event is generated when an indication is received from the server.
    GattIndication(AttributeValue),

    /// This event is generated when an notification is received from the server.
    GattNotification(AttributeValue),

    /// This event is generated when a GATT client procedure completes either with error or
    /// successfully.
    GattProcedureComplete(GattProcedureComplete),

    /// This event is generated when an Error Response is received from the server. The error
    /// response can be given by the server at the end of one of the GATT discovery procedures. This
    /// does not mean that the procedure ended with an error, but this error event is part of the
    /// procedure itself.
    AttErrorResponse(AttErrorResponse),

    /// This event can be generated during a "Discover Characteristics by UUID" procedure or a "Read
    /// using Characteristic UUID" procedure. The attribute value will be a service declaration as
    /// defined in Bluetooth Core v4.0 spec, Vol 3, Part G, section 3.3.1), when a "Discover
    /// Characteristics By UUID" has been started. It will be the value of the Characteristic if a
    /// "Read using Characteristic UUID" has been performed.
    ///
    /// See the Bluetooth Core v4.1 spec, Vol 3, Part G, section 4.6.2 (discover characteristics by
    /// UUID), and section 4.8.2 (read using characteristic using UUID).
    GattDiscoverOrReadCharacteristicByUuidResponse(AttributeValue),

    /// This event is given to the application when a write request, write command or signed write
    /// command is received by the server from the client. This event will be given to the
    /// application only if the event bit for this event generation is set when the characteristic
    /// was added. When this event is received, the application has to check whether the value being
    /// requested for write is allowed to be written and respond with a GATT Write Response. If the
    /// write is rejected by the application, then the value of the attribute will not be
    /// modified. In case of a write request, an error response will be sent to the client, with the
    /// error code as specified by the application. In case of write/signed write commands, no
    /// response is sent to the client but the attribute is not modified.
    ///
    /// See the Bluetooth Core v4.1 spec, Vol 3, Part F, section 3.4.5.
    AttWritePermitRequest(AttributeValue),

    /// An unknown event was sent. Includes the event code but no other information about the
    /// event. The remaining data from the event is lost.
    UnknownEvent(u16),
}

/// Newtype for a connection handle. For several events, the only data is a connection handle. Other
/// events include a connection handle as one of the parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionHandle(pub u16);

macro_rules! require_len {
    ($left:expr, $right:expr) => {
        if $left.len() != $right {
            return Err(hci::event::Error::BadLength($left.len(), $right));
        }
    };
}

macro_rules! require_len_at_least {
    ($left:expr, $right:expr) => {
        if $left.len() < $right {
            return Err(hci::event::Error::BadLength($left.len(), $right));
        }
    };
}

fn first_16(buffer: &[u8]) -> &[u8] {
    if buffer.len() < 16 {
        &buffer
    } else {
        &buffer[..16]
    }
}

impl hci::event::VendorEvent for BlueNRGEvent {
    type Error = Error;

    fn new(buffer: &[u8]) -> Result<BlueNRGEvent, hci::event::Error<Error>> {
        require_len_at_least!(buffer, 2);

        let event_code = LittleEndian::read_u16(&buffer[0..=1]);
        match event_code {
            0x0001 => Ok(BlueNRGEvent::HalInitialized(to_hal_initialized(buffer)?)),
            0x0002 => {
                #[cfg(feature = "ms")]
                {
                    Ok(BlueNRGEvent::EventsLost(to_lost_event(buffer)?))
                }

                #[cfg(not(feature = "ms"))]
                {
                    Err(hci::event::Error::Vendor(Error::UnknownEvent(event_code)))
                }
            }
            0x0003 => {
                #[cfg(feature = "ms")]
                {
                    Ok(BlueNRGEvent::CrashReport(to_crash_report(buffer)?))
                }

                #[cfg(not(feature = "ms"))]
                {
                    Err(hci::event::Error::Vendor(Error::UnknownEvent(event_code)))
                }
            }
            0x0400 => Ok(BlueNRGEvent::GapLimitedDiscoverable),
            0x0401 => Ok(BlueNRGEvent::GapPairingComplete(to_gap_pairing_complete(
                buffer,
            )?)),
            0x0402 => Ok(BlueNRGEvent::GapPassKeyRequest(to_conn_handle(buffer)?)),
            0x0403 => Ok(BlueNRGEvent::GapAuthorizationRequest(to_conn_handle(
                buffer,
            )?)),
            0x0404 => Ok(BlueNRGEvent::GapSlaveSecurityInitiated),
            0x0405 => Ok(BlueNRGEvent::GapBondLost),
            0x0406 => Ok(BlueNRGEvent::GapDeviceFound(to_gap_device_found(buffer)?)),
            0x0407 => Ok(BlueNRGEvent::GapProcedureComplete(
                to_gap_procedure_complete(buffer)?,
            )),
            0x0408 => {
                #[cfg(feature = "ms")]
                {
                    Ok(BlueNRGEvent::GapAddressNotResolved(to_conn_handle(buffer)?))
                }

                #[cfg(not(feature = "ms"))]
                {
                    Ok(BlueNRGEvent::GapReconnectionAddress(
                        to_gap_reconnection_address(buffer)?,
                    ))
                }
            }
            0x0800 => Ok(BlueNRGEvent::L2CapConnectionUpdateResponse(
                to_l2cap_connection_update_response(buffer)?,
            )),
            0x0801 => Ok(BlueNRGEvent::L2CapProcedureTimeout(
                to_l2cap_procedure_timeout(buffer)?,
            )),
            0x0802 => Ok(BlueNRGEvent::L2CapConnectionUpdateRequest(
                to_l2cap_connection_update_request(buffer)?,
            )),
            0x0C01 => Ok(BlueNRGEvent::GattAttributeModified(
                to_gatt_attribute_modified(buffer)?,
            )),
            0x0C02 => Ok(BlueNRGEvent::GattProcedureTimeout(to_conn_handle(buffer)?)),
            0x0C03 => Ok(BlueNRGEvent::AttExchangeMtuResponse(
                to_att_exchange_mtu_resp(buffer)?,
            )),
            0x0C04 => Ok(BlueNRGEvent::AttFindInformationResponse(
                to_att_find_information_response(buffer)?,
            )),
            0x0C05 => Ok(BlueNRGEvent::AttFindByTypeValueResponse(
                to_att_find_by_value_type_response(buffer)?,
            )),
            0x0C06 => Ok(BlueNRGEvent::AttReadByTypeResponse(
                to_att_read_by_type_response(buffer)?,
            )),
            0x0C07 => Ok(BlueNRGEvent::AttReadResponse(to_att_read_response(buffer)?)),
            0x0C08 => Ok(BlueNRGEvent::AttReadBlobResponse(to_att_read_response(
                buffer,
            )?)),
            0x0C09 => Ok(BlueNRGEvent::AttReadMultipleResponse(to_att_read_response(
                buffer,
            )?)),
            0x0C0A => Ok(BlueNRGEvent::AttReadByGroupTypeResponse(
                to_att_read_by_group_type_response(buffer)?,
            )),
            0x0C0C => Ok(BlueNRGEvent::AttPrepareWriteResponse(
                to_att_prepare_write_response(buffer)?,
            )),
            0x0C0D => Ok(BlueNRGEvent::AttExecuteWriteResponse(to_conn_handle(
                buffer,
            )?)),
            0x0C0E => Ok(BlueNRGEvent::GattIndication(to_attribute_value(buffer)?)),
            0x0C0F => Ok(BlueNRGEvent::GattNotification(to_attribute_value(buffer)?)),
            0x0C10 => Ok(BlueNRGEvent::GattProcedureComplete(
                to_gatt_procedure_complete(buffer)?,
            )),
            0x0C11 => Ok(BlueNRGEvent::AttErrorResponse(to_att_error_response(
                buffer,
            )?)),
            0x0C12 => Ok(
                BlueNRGEvent::GattDiscoverOrReadCharacteristicByUuidResponse(to_attribute_value(
                    buffer,
                )?),
            ),
            0x0C13 => Ok(BlueNRGEvent::AttWritePermitRequest(
                to_write_permit_request(buffer)?,
            )),
            _ => Err(hci::event::Error::Vendor(Error::UnknownEvent(event_code))),
        }
    }
}

/// Potential reasons the controller sent the HalInitialized event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResetReason {
    /// Firmware started properly
    Normal,
    /// Updater mode entered because of Aci_Updater_Start command
    UpdaterAci,
    /// Updater mode entered because of a bad BLUE flag
    UpdaterBadFlag,
    /// Updater mode entered with IRQ pin
    UpdaterPin,
    /// Reset caused by watchdog
    Watchdog,
    /// Reset due to lockup
    Lockup,
    /// Brownout reset
    Brownout,
    /// Reset caused by a crash (NMI or Hard Fault)
    Crash,
    /// Reset caused by an ECC error
    EccError,
}

impl TryFrom<u8> for ResetReason {
    type Error = Error;

    fn try_from(value: u8) -> Result<ResetReason, Self::Error> {
        match value {
            1 => Ok(ResetReason::Normal),
            2 => Ok(ResetReason::UpdaterAci),
            3 => Ok(ResetReason::UpdaterBadFlag),
            4 => Ok(ResetReason::UpdaterPin),
            5 => Ok(ResetReason::Watchdog),
            6 => Ok(ResetReason::Lockup),
            7 => Ok(ResetReason::Brownout),
            8 => Ok(ResetReason::Crash),
            9 => Ok(ResetReason::EccError),
            _ => Err(Error::UnknownResetReason(value)),
        }
    }
}

/// Convert a buffer to the HalInitialized BlueNRGEvent.
///
/// # Errors
///
/// - Returns a BadLength HCI error if the buffer is not exactly 3 bytes long
///
/// - Returns a UnknownResetReason BlueNRG error if the reset reason is not recognized.
fn to_hal_initialized(buffer: &[u8]) -> Result<ResetReason, hci::event::Error<Error>> {
    require_len!(buffer, 3);

    Ok(buffer[2]
        .try_into()
        .map_err(|e| hci::event::Error::Vendor(e))?)
}

#[cfg(feature = "ms")]
bitflags! {
    /// Bitfield for the EventsLost event. Each bit indicates a different type of event that was not
    /// handled.
    #[derive(Default)]
    pub struct EventFlags: u64 {
        /// HCI Event: Disconnection complete
        const DISCONNECTION_COMPLETE = 1 << 0;
        /// HCI Event: Encryption change
        const ENCRYPTION_CHANGE = 1 << 1;
        /// HCI Event: Read Remote Version Complete
        const READ_REMOTE_VERSION_COMPLETE = 1 << 2;
        /// HCI Event: Command Complete
        const COMMAND_COMPLETE = 1 << 3;
        /// HCI Event: Command Status
        const COMMAND_STATUS = 1 << 4;
        /// HCI Event: Hardware Error
        const HARDWARE_ERROR = 1 << 5;
        /// HCI Event: Number of completed packets
        const NUMBER_OF_COMPLETED_PACKETS = 1 << 6;
        /// HCI Event: Encryption key refresh complete
        const ENCRYPTION_KEY_REFRESH = 1 << 7;
        /// BlueNRG-MS Event: HAL Initialized
        const HAL_INITIALIZED = 1 << 8;
        /// BlueNRG Event: GAP Set Limited Discoverable complete
        const GAP_SET_LIMITED_DISCOVERABLE = 1 << 9;
        /// BlueNRG Event: GAP Pairing complete
        const GAP_PAIRING_COMPLETE = 1 << 10;
        /// BlueNRG Event: GAP Pass Key Request
        const GAP_PASS_KEY_REQUEST = 1 << 11;
        /// BlueNRG Event: GAP Authorization Request
        const GAP_AUTHORIZATION_REQUEST = 1 << 12;
        /// BlueNRG Event: GAP Slave Security Initiated
        const GAP_SLAVE_SECURITY_INITIATED = 1 << 13;
        /// BlueNRG Event: GAP Bond Lost
        const GAP_BOND_LOST = 1 << 14;
        /// BlueNRG Event: GAP Procedure Complete
        const GAP_PROCEDURE_COMPLETE = 1 << 15;
        /// BlueNRG-MS Event: GAP Address Not Resolved
        const GAP_ADDRESS_NOT_RESOLVED = 1 << 16;
        /// BlueNRG Event: L2Cap Connection Update Response
        const L2CAP_CONNECTION_UPDATE_RESPONSE = 1 << 17;
        /// BlueNRG Event: L2Cap Procedure Timeout
        const L2CAP_PROCEDURE_TIMEOUT = 1 << 18;
        /// BlueNRG Event: L2Cap Connection Update Request
        const L2CAP_CONNECTION_UPDATE_REQUEST = 1 << 19;
        /// BlueNRG Event: ATT Attribute modified
        const GATT_ATTRIBUTE_MODIFIED = 1 << 20;
        /// BlueNRG Event: ATT timeout
        const GATT_PROCEDURE_TIMEOUT = 1 << 21;
        /// BlueNRG Event: Exchange MTU Response
        const ATT_EXCHANGE_MTU_RESPONSE = 1 << 22;
        /// BlueNRG Event: Find information response
        const ATT_FIND_INFORMATION_RESPONSE = 1 << 23;
        /// BlueNRG Event: Find by type value response
        const ATT_FIND_BY_TYPE_VALUE_RESPONSE = 1 << 24;
        /// BlueNRG Event: Find read by type response
        const ATT_READ_BY_TYPE_RESPONSE = 1 << 25;
        /// BlueNRG Event: Read response
        const ATT_READ_RESPONSE = 1 << 26;
        /// BlueNRG Event: Read blob response
        const ATT_READ_BLOB_RESPONSE = 1 << 27;
        /// BlueNRG Event: Read multiple response
        const ATT_READ_MULTIPLE_RESPONSE = 1 << 28;
        /// BlueNRG Event: Read by group type response
        const ATT_READ_BY_GROUP_TYPE_RESPONSE = 1 << 29;
        /// BlueNRG Event: ATT Write Response
        const ATT_WRITE_RESPONSE = 1 << 30;
        /// BlueNRG Event: Prepare Write Response
        const ATT_PREPARE_WRITE_RESPONSE = 1 << 31;
        /// BlueNRG Event: Execute write response
        const ATT_EXECUTE_WRITE_RESPONSE = 1 << 32;
        /// BlueNRG Event: Indication received from server
        const GATT_INDICATION = 1 << 33;
        /// BlueNRG Event: Notification received from server
        const GATT_NOTIFICATION = 1 << 34;
        /// BlueNRG Event: ATT Procedure complete
        const GATT_PROCEDURE_COMPLETE = 1 << 35;
        /// BlueNRG Event: Error response received from server
        const GATT_ERROR_RESPONSE = 1 << 36;
        /// BlueNRG Event: Response to either "Discover Characteristic by UUID" or "Read
        /// Characteristic by UUID" request
        const GATT_DISCOVER_OR_READ_CHARACTERISTIC_BY_UUID_RESPONSE = 1 << 37;
        /// BlueNRG Event: Write request received by server
        const GATT_WRITE_PERMIT_REQUEST = 1 << 38;
        /// BlueNRG Event: Read request received by server
        const GATT_READ_PERMIT_REQUEST = 1 << 39;
        /// BlueNRG Event: Read multiple request received by server
        const GATT_READ_MULTIPLE_PERMIT_REQUEST = 1 << 40;
        /// BlueNRG-MS Event: TX Pool available event missed
        const GATT_TX_POOL_AVAILABLE = 1 << 41;
        /// BlueNRG-MS Event: Server confirmation
        const GATT_SERVER_RX_CONFIRMATION = 1 << 42;
        /// BlueNRG-MS Event: Prepare write permit request
        const GATT_PREPARE_WRITE_PERMIT_REQUEST = 1 << 43;
        /// BlueNRG-MS Event: Link Layer connection complete
        const LINK_LAYER_CONNECTION_COMPLETE = 1 << 44;
        /// BlueNRG-MS Event: Link Layer advertising report
        const LINK_LAYER_ADVERTISING_REPORT = 1 << 45;
        /// BlueNRG-MS Event: Link Layer connection update complete
        const LINK_LAYER_CONNECTION_UPDATE_COMPLETE = 1 << 46;
        /// BlueNRG-MS Event: Link Layer read remote used features
        const LINK_LAYER_READ_REMOTE_USED_FEATURES = 1 << 47;
        /// BlueNRG-MS Event: Link Layer long-term key request
        const LINK_LAYER_LTK_REQUEST = 1 << 48;
    }
}

/// Convert a buffer to the EventsLost BlueNRGEvent.
///
/// # Errors
///
/// - Returns a BadLength HCI error if the buffer is not exactly 10 bytes long
///
/// - Returns BadEventFlags if a bit is set that does not represent a lost event.
#[cfg(feature = "ms")]
fn to_lost_event(buffer: &[u8]) -> Result<EventFlags, hci::event::Error<Error>> {
    require_len!(buffer, 10);

    let bits = LittleEndian::read_u64(&buffer[2..]);
    EventFlags::from_bits(bits).ok_or(hci::event::Error::Vendor(Error::BadEventFlags(bits)))
}

/// The maximum length of [`debug_data`] in [`FaultData`]. The maximum length of an event is 255
/// bytes, and the non-variable data of the event takes up 40 bytes.
#[cfg(feature = "ms")]
pub const MAX_DEBUG_DATA_LEN: usize = 215;

/// Specific reason for the fault reported with FaultData.
#[cfg(feature = "ms")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CrashReason {
    /// The controller reset because an assertion failed.
    Assertion,

    /// The controller reset because of an NMI fault.
    NmiFault,

    /// The controller reset because of a hard fault.
    HardFault,
}

#[cfg(feature = "ms")]
impl TryFrom<u8> for CrashReason {
    type Error = Error;

    fn try_from(value: u8) -> Result<CrashReason, Self::Error> {
        match value {
            0 => Ok(CrashReason::Assertion),

            // The documentation is conflicting for the numeric value of NMI Fault. The
            // CubeExpansion source code says 1, but the user manual says 6.
            1 | 6 => Ok(CrashReason::NmiFault),

            // The documentation is conflicting for the numeric value of hard Fault. The
            // CubeExpansion source code says 2, but the user manual says 7.
            2 | 7 => Ok(CrashReason::HardFault),
            _ => Err(Error::UnknownCrashReason(value)),
        }
    }
}

/// Fault data reported after a crash.
#[cfg(feature = "ms")]
#[derive(Clone, Copy)]
pub struct FaultData {
    /// Fault reason.
    pub reason: CrashReason,

    /// MCP SP register
    pub sp: u32,
    /// MCU R0 register
    pub r0: u32,
    /// MCU R1 register
    pub r1: u32,
    /// MCU R2 register
    pub r2: u32,
    /// MCU R3 register
    pub r3: u32,
    /// MCU R12 register
    pub r12: u32,
    /// MCU LR register
    pub lr: u32,
    /// MCU PC register
    pub pc: u32,
    /// MCU xPSR register
    pub xpsr: u32,

    /// Number of valid bytes in debug_data
    pub debug_data_len: usize,

    /// Additional crash dump data
    pub debug_data: [u8; MAX_DEBUG_DATA_LEN],
}

#[cfg(feature = "ms")]
impl Debug for FaultData {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "FaultData {{ reason: {:?}, sp: {:x}, r0: {:x}, r1: {:x}, r2: {:x}, r3: {:x}, ",
            self.reason, self.sp, self.r0, self.r1, self.r2, self.r3
        )?;
        write!(
            f,
            "r12: {:x}, lr: {:x}, pc: {:x}, xpsr: {:x}, debug_data: [",
            self.r12, self.lr, self.pc, self.xpsr
        )?;
        for byte in &self.debug_data[..self.debug_data_len] {
            write!(f, " {:x}", byte)?;
        }
        write!(f, " ] }}")
    }
}

#[cfg(feature = "ms")]
fn to_crash_report(buffer: &[u8]) -> Result<FaultData, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 40);

    let debug_data_len = buffer[39] as usize;
    require_len!(buffer, 40 + debug_data_len);

    let mut fault_data = FaultData {
        reason: buffer[2].try_into().map_err(hci::event::Error::Vendor)?,
        sp: LittleEndian::read_u32(&buffer[3..]),
        r0: LittleEndian::read_u32(&buffer[7..]),
        r1: LittleEndian::read_u32(&buffer[11..]),
        r2: LittleEndian::read_u32(&buffer[15..]),
        r3: LittleEndian::read_u32(&buffer[19..]),
        r12: LittleEndian::read_u32(&buffer[23..]),
        lr: LittleEndian::read_u32(&buffer[27..]),
        pc: LittleEndian::read_u32(&buffer[31..]),
        xpsr: LittleEndian::read_u32(&buffer[35..]),
        debug_data_len: debug_data_len,
        debug_data: [0; MAX_DEBUG_DATA_LEN],
    };
    fault_data.debug_data[..debug_data_len].copy_from_slice(&buffer[40..]);

    Ok(fault_data)
}

macro_rules! require_l2cap_event_data_len {
    ($left:expr, $right:expr) => {
        let actual = $left[4];
        if actual != $right {
            return Err(hci::event::Error::Vendor(Error::BadL2CapDataLength(
                actual, $right,
            )));
        }
    };
}

macro_rules! require_l2cap_len {
    ($actual:expr, $expected:expr) => {
        if $actual != $expected {
            return Err(hci::event::Error::Vendor(Error::BadL2CapLength(
                $actual, $expected,
            )));
        }
    };
}

/// This event is generated when the master responds to the L2CAP connection update request packet.
/// For more info see CONNECTION PARAMETER UPDATE RESPONSE and COMMAND REJECT in Bluetooth Core v4.0
/// spec.
#[derive(Copy, Clone, Debug)]
pub struct L2CapConnectionUpdateResponse {
    /// The connection handle related to the event
    pub conn_handle: ConnectionHandle,

    /// The result of the update request, including details about the result.
    pub result: L2CapConnectionUpdateResult,
}

/// Reasons why an L2CAP command was rejected. see the Bluetooth specification, Vol 3, Part A,
/// Section 4.1 (versions 4.1, 4.2, and 5.0).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum L2CapRejectionReason {
    /// The controller sent an unknown command
    CommandNotUnderstood,
    /// When multiple commands are included in an L2CAP packet and the packet exceeds the signaling
    /// MTU (MTUsig) of the receiver, a single Command Reject packet shall be sent in response.
    SignalingMtuExceeded,
    /// Invalid CID in request
    InvalidCid,
}

impl TryFrom<u16> for L2CapRejectionReason {
    type Error = Error;

    fn try_from(value: u16) -> Result<L2CapRejectionReason, Self::Error> {
        match value {
            0 => Ok(L2CapRejectionReason::CommandNotUnderstood),
            1 => Ok(L2CapRejectionReason::SignalingMtuExceeded),
            2 => Ok(L2CapRejectionReason::InvalidCid),
            _ => Err(Error::BadL2CapRejectionReason(value)),
        }
    }
}

/// Potential results that can be used in the L2CAP connection update response.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum L2CapConnectionUpdateResult {
    /// The update request was rejected. The code indicates the reason for the rejection.
    CommandRejected(L2CapRejectionReason),

    /// The L2CAP connection update response is valid. The code indicates if the parameters were
    /// rejected.
    ParametersRejected,

    /// The L2CAP connection update response is valid. The code indicates if the parameters were
    /// updated.
    ParametersUpdated,
}

fn to_l2cap_connection_update_accepted_result(
    value: u16,
) -> Result<L2CapConnectionUpdateResult, Error> {
    match value {
        0x0000 => Ok(L2CapConnectionUpdateResult::ParametersUpdated),
        0x0001 => Ok(L2CapConnectionUpdateResult::ParametersRejected),
        _ => {
            return Err(Error::BadL2CapConnectionResponseResult(value));
        }
    }
}

fn extract_l2cap_connection_update_response_result(
    buffer: &[u8],
) -> Result<L2CapConnectionUpdateResult, Error> {
    match buffer[5] {
        0x01 => Ok(L2CapConnectionUpdateResult::CommandRejected(
            LittleEndian::read_u16(&buffer[9..]).try_into()?,
        )),
        0x13 => to_l2cap_connection_update_accepted_result(LittleEndian::read_u16(&buffer[9..])),
        _ => Err(Error::BadL2CapConnectionResponseCode(buffer[5])),
    }
}

fn to_l2cap_connection_update_response(
    buffer: &[u8],
) -> Result<L2CapConnectionUpdateResponse, hci::event::Error<Error>> {
    require_len!(buffer, 11);
    require_l2cap_event_data_len!(buffer, 6);
    require_l2cap_len!(LittleEndian::read_u16(&buffer[7..]), 2);

    Ok(L2CapConnectionUpdateResponse {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        result: extract_l2cap_connection_update_response_result(buffer)
            .map_err(hci::event::Error::Vendor)?,
    })
}

/// This event is generated when the master does not respond to the connection update request within
/// 30 seconds.
#[derive(Copy, Clone, Debug)]
pub struct L2CapProcedureTimeout {
    /// The connection handle related to the event.
    pub conn_handle: ConnectionHandle,
}

fn to_l2cap_procedure_timeout(buffer: &[u8]) -> Result<ConnectionHandle, hci::event::Error<Error>> {
    require_len!(buffer, 5);
    require_l2cap_event_data_len!(buffer, 0);

    Ok(ConnectionHandle(LittleEndian::read_u16(&buffer[2..])))
}

/// The event is given by the L2CAP layer when a connection update request is received from the
/// slave.  The application has to respond by calling
/// aci_l2cap_connection_parameter_update_response().
///
/// Defined in Vol 3, Part A, section 4.20 of the Bluetooth specification. The definition is the
/// same for version 4.1, 4.2, and 5.0.
#[derive(Copy, Clone, Debug)]
pub struct L2CapConnectionUpdateRequest {
    /// Handle of the connection for which the connection update request has been received.  The
    /// same handle has to be returned while responding to the event with the command
    /// aci_l2cap_connection_parameter_update_response().
    pub conn_handle: ConnectionHandle,

    /// This is the identifier which associates the request to the response. The same identifier has
    /// to be returned by the upper layer in the command
    /// aci_l2cap_connection_parameter_update_response().
    pub identifier: u8,

    /// Defines minimum value for the connection interval in the following manner:
    /// `connIntervalMin = Interval Min * 1.25 ms`. Interval Min range: 6 to 3200 frames where 1
    /// frame is 1.25 ms and equivalent to 2 BR/EDR slots. Values outside the range are reserved for
    /// future use. Interval Min shall be less than or equal to Interval Max.
    pub interval_min: u16,

    /// Defines maximum value for the connection interval in the following manner:
    /// `connIntervalMax = Interval Max * 1.25 ms`. Interval Max range: 6 to 3200 frames. Values
    /// outside the range are reserved for future use. Interval Max shall be equal to or greater
    /// than the Interval Min.
    pub interval_max: u16,

    /// Defines the slave latency parameter (as number of LL connection events) in the following
    /// manner: `connSlaveLatency = Slave Latency`. The Slave Latency field shall have a value in
    /// the range of 0 to ((connSupervisionTimeout / (connIntervalMax*2)) -1). The Slave Latency
    /// field shall be less than 500.
    pub slave_latency: u16,

    /// Defines connection timeout parameter in the following manner: `connSupervisionTimeout =
    /// Timeout Multiplier * 10 ms`. The Timeout Multiplier field shall have a value in the range of
    /// 10 to 3200.
    pub timeout_mult: u16,
}

fn outside_interval_range(value: u16) -> bool {
    value < 6 || value > 3200
}

fn to_l2cap_connection_update_request(
    buffer: &[u8],
) -> Result<L2CapConnectionUpdateRequest, hci::event::Error<Error>> {
    require_len!(buffer, 16);
    require_l2cap_event_data_len!(buffer, 11);
    require_l2cap_len!(LittleEndian::read_u16(&buffer[6..]), 8);

    let interval_min = LittleEndian::read_u16(&buffer[8..]);
    let interval_max = LittleEndian::read_u16(&buffer[10..]);
    if outside_interval_range(interval_min) || outside_interval_range(interval_max)
        || interval_min > interval_max
    {
        return Err(hci::event::Error::Vendor(
            Error::BadL2CapConnectionUpdateRequestInterval(interval_min, interval_max),
        ));
    }

    let timeout_mult = LittleEndian::read_u16(&buffer[14..]);
    if timeout_mult < 10 || timeout_mult > 3200 {
        return Err(hci::event::Error::Vendor(
            Error::BadL2CapConnectionUpdateRequestTimeoutMult(timeout_mult),
        ));
    }

    let slave_latency = LittleEndian::read_u16(&buffer[12..]);

    // The maximum allowed slave latency is defined by ((supervision_timeout / (2 *
    // connection_interval_max)) - 1), where
    //   supervision_timeout = 10 * timeout_mult
    //   connection_interval_max = 1.25 * interval_max
    // This simplifies to the expression below. Regardless of the other values, the slave latency
    // must be less than 500.
    let slave_latency_limit = min(500, (4 * timeout_mult) / interval_max - 1);
    if slave_latency >= slave_latency_limit {
        return Err(hci::event::Error::Vendor(
            Error::BadL2CapConnectionUpdateRequestLatency(slave_latency, slave_latency_limit),
        ));
    }

    Ok(L2CapConnectionUpdateRequest {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        identifier: buffer[5],
        interval_min: interval_min,
        interval_max: interval_max,
        slave_latency: slave_latency,
        timeout_mult: timeout_mult,
    })
}

/// This event is generated when the pairing process has completed successfully or a pairing
/// procedure timeout has occurred or the pairing has failed. This is to notify the application that
/// we have paired with a remote device so that it can take further actions or to notify that a
/// timeout has occurred so that the upper layer can decide to disconnect the link.
#[derive(Copy, Clone, Debug)]
pub struct GapPairingComplete {
    /// Connection handle on which the pairing procedure completed
    pub conn_handle: ConnectionHandle,

    /// Reason the pairing is complete.
    pub status: GapPairingStatus,
}

/// Reasons the GAP Pairing Complete event was generated.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GapPairingStatus {
    /// Pairing with a remote device was successful
    Success,
    /// The SMP timeout has elapsed and no further SMP commands will be processed until reconnection
    Timeout,
    /// The pairing failed with the remote device.
    Failed,
}

impl TryFrom<u8> for GapPairingStatus {
    type Error = Error;

    fn try_from(value: u8) -> Result<GapPairingStatus, Self::Error> {
        match value {
            0 => Ok(GapPairingStatus::Success),
            1 => Ok(GapPairingStatus::Timeout),
            2 => Ok(GapPairingStatus::Failed),
            _ => Err(Error::BadGapPairingStatus(value)),
        }
    }
}

fn to_gap_pairing_complete(buffer: &[u8]) -> Result<GapPairingComplete, hci::event::Error<Error>> {
    require_len!(buffer, 5);
    Ok(GapPairingComplete {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        status: buffer[4].try_into().map_err(hci::event::Error::Vendor)?,
    })
}

fn to_conn_handle(buffer: &[u8]) -> Result<ConnectionHandle, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 4);
    Ok(ConnectionHandle(LittleEndian::read_u16(&buffer[2..])))
}

/// The event is given by the GAP layer to the upper layers when a device is discovered during
/// scanning as a consequence of one of the GAP procedures started by the upper layers.
#[derive(Copy, Clone, Debug)]
pub struct GapDeviceFound {
    /// Type of event
    pub event: GapDeviceFoundEvent,

    /// Address of the peer device found during scanning
    pub bdaddr: BdAddr,

    /// Length of significant data
    pub data_len: usize,

    /// Advertising or scan response data.
    pub data: [u8; 31],

    /// Received signal strength indicator (range: -127 - 20)
    pub rssi: i8,
}

/// Potential values for the GAP Device Found event type.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GapDeviceFoundEvent {
    /// Connectable undirected advertising
    Advertisement,
    /// Connectable directed advertising
    DirectAdvertisement,
    /// Scannable undirected advertising
    Scan,
    /// Non connectable undirected advertising
    NonConnectableAdvertisement,
    /// Scan Response
    ScanResponse,
}

impl TryFrom<u8> for GapDeviceFoundEvent {
    type Error = Error;

    fn try_from(value: u8) -> Result<GapDeviceFoundEvent, Self::Error> {
        match value {
            0 => Ok(GapDeviceFoundEvent::Advertisement),
            1 => Ok(GapDeviceFoundEvent::DirectAdvertisement),
            2 => Ok(GapDeviceFoundEvent::Scan),
            3 => Ok(GapDeviceFoundEvent::NonConnectableAdvertisement),
            4 => Ok(GapDeviceFoundEvent::ScanResponse),
            _ => Err(Error::BadGapDeviceFoundEvent(value)),
        }
    }
}

/// Newtype for BDADDR buffer.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BdAddrBuffer(pub [u8; 6]);

/// Potential values for BDADDR
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BdAddr {
    /// Public address.
    Public(BdAddrBuffer),

    /// Random address.
    Random(BdAddrBuffer),
}

fn to_bdaddr(bd_addr_type: u8, addr: BdAddrBuffer) -> Result<BdAddr, Error> {
    match bd_addr_type {
        0 => Ok(BdAddr::Public(addr)),
        1 => Ok(BdAddr::Random(addr)),
        _ => Err(Error::BadGapBdAddrType(bd_addr_type)),
    }
}

fn to_gap_device_found(buffer: &[u8]) -> Result<GapDeviceFound, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 12);

    let data_len = buffer[10] as usize;
    require_len!(buffer, 12 + data_len);

    const RSSI_UNAVAILABLE: i8 = 127;
    let rssi = unsafe { mem::transmute::<u8, i8>(buffer[buffer.len() - 1]) };
    if rssi == RSSI_UNAVAILABLE {
        return Err(hci::event::Error::Vendor(Error::GapRssiUnavailable));
    }

    let mut addr = BdAddrBuffer([0; 6]);
    addr.0.copy_from_slice(&buffer[4..10]);
    let mut event = GapDeviceFound {
        event: buffer[2].try_into().map_err(hci::event::Error::Vendor)?,
        bdaddr: to_bdaddr(buffer[3], addr).map_err(hci::event::Error::Vendor)?,
        data_len: data_len,
        data: [0; 31],
        rssi: rssi,
    };
    event.data[..event.data_len].copy_from_slice(&buffer[11..buffer.len() - 1]);

    Ok(event)
}

/// This event is sent by the GAP to the upper layers when a procedure previously started has been
/// terminated by the upper layer or has completed for any other reason
#[derive(Copy, Clone, Debug)]
pub struct GapProcedureComplete {
    /// Type of procedure that completed
    pub procedure: GapProcedure,
    /// Status of the procedure
    pub status: GapProcedureStatus,
}

/// Maximum length of the name returned in the NameDiscovery procedure.
pub const MAX_NAME_LEN: usize = 248;

/// Newtype for the name buffer returned after successful NameDiscovery.
#[derive(Copy, Clone)]
pub struct NameBuffer(pub [u8; MAX_NAME_LEN]);

impl Debug for NameBuffer {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        first_16(&self.0).fmt(f)
    }
}

impl PartialEq<NameBuffer> for NameBuffer {
    fn eq(&self, other: &NameBuffer) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }

        for (a, b) in self.0.iter().zip(other.0.iter()) {
            if a != b {
                return false;
            }
        }

        return true;
    }
}

/// Procedures whose completion may be reported by GapProcedureComplete.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GapProcedure {
    /// See section 9.2.5, Vol 3, Part C
    LimitedDiscovery,
    /// See section 9.2.6, Vol 3, Part C
    GeneralDiscovery,
    /// See section 9.2.7, Vol 3, Part C. Contains the number of valid bytes and buffer with enough
    /// space for the maximum length of the name that can be retuned.
    NameDiscovery(usize, NameBuffer),
    /// See section 9.3.5, Vol 3, Part C
    AutoConnectionEstablishment,
    /// See section 9.3.6, Vol 3, Part C. Contains the reconnection address.
    GeneralConnectionEstablishment(BdAddrBuffer),
    /// See section 9.3.7, Vol 3, Part C
    SelectiveConnectionEstablishment,
    /// See section 9.3.8, Vol 3, Part C
    DirectConnectionEstablishment,
}

/// Possible results of a procedure
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GapProcedureStatus {
    /// BLE Status Success
    Success,
    /// BLE Status Failed
    Failed,
    /// Procedure failed due to authentication requirements
    AuthFailure,
}

impl TryFrom<u8> for GapProcedureStatus {
    type Error = Error;

    fn try_from(value: u8) -> Result<GapProcedureStatus, Self::Error> {
        match value {
            0x00 => Ok(GapProcedureStatus::Success),
            0x41 => Ok(GapProcedureStatus::Failed),
            0x05 => Ok(GapProcedureStatus::AuthFailure),
            _ => Err(Error::BadGapProcedureStatus(value)),
        }
    }
}

fn to_gap_procedure_complete(
    buffer: &[u8],
) -> Result<GapProcedureComplete, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 4);

    let procedure = match buffer[2] {
        0x01 => GapProcedure::LimitedDiscovery,
        0x02 => GapProcedure::GeneralDiscovery,
        0x04 => {
            require_len_at_least!(buffer, 5);
            let name_len = buffer.len() - 4;
            let mut name = NameBuffer([0; MAX_NAME_LEN]);
            name.0[..name_len].copy_from_slice(&buffer[4..]);

            GapProcedure::NameDiscovery(name_len, name)
        }
        0x08 => GapProcedure::AutoConnectionEstablishment,
        0x10 => {
            require_len!(buffer, 10);
            let mut addr = BdAddrBuffer([0; 6]);
            addr.0.copy_from_slice(&buffer[4..10]);
            GapProcedure::GeneralConnectionEstablishment(addr)
        }
        0x20 => GapProcedure::SelectiveConnectionEstablishment,
        0x40 => GapProcedure::DirectConnectionEstablishment,
        _ => {
            return Err(hci::event::Error::Vendor(Error::BadGapProcedure(buffer[2])));
        }
    };

    Ok(GapProcedureComplete {
        procedure: procedure,
        status: buffer[3].try_into().map_err(hci::event::Error::Vendor)?,
    })
}

#[cfg(not(feature = "ms"))]
fn to_gap_reconnection_address(buffer: &[u8]) -> Result<BdAddrBuffer, hci::event::Error<Error>> {
    require_len!(buffer, 8);
    let mut addr = BdAddrBuffer([0; 6]);
    addr.0.copy_from_slice(&buffer[2..]);
    Ok(addr)
}

/// This event is generated to the application by the ATT server when a client modifies any
/// attribute on the server, as consequence of one of the following ATT procedures:
/// - write without response
/// - signed write without response
/// - write characteristic value
/// - write long characteristic value
/// - reliable write
#[derive(Copy, Clone)]
pub struct GattAttributeModified {
    /// The connection handle which modified the attribute
    pub conn_handle: ConnectionHandle,
    ///  Handle of the attribute that was modified
    pub attr_handle: AttributeHandle,

    /// Offset of the reported value inside the attribute.
    #[cfg(feature = "ms")]
    pub offset: usize,

    /// If the entire value of the attribute does not fit inside a single GattAttributeModified
    /// event, this is true to notify that other GattAttributeModified events will follow to report
    /// the remaining value.
    #[cfg(feature = "ms")]
    pub continued: bool,

    /// Number of valid bytes in |data|.
    data_len: usize,
    /// The new attribute value, starting from the given offset. If compiling with "ms" support, the
    /// offset is 0.
    data_buf: [u8; MAX_ATTRIBUTE_LEN],
}

impl GattAttributeModified {
    /// Returns the valid attribute data returned by the ATT attribute modified event as a slice of
    /// bytes.
    pub fn data(&self) -> &[u8] {
        &self.data_buf[..self.data_len]
    }
}

/// Newtype for an attribute handle. These handles are IDs, not general integers, and should not be
/// manipulated as such.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AttributeHandle(pub u16);

// Defines the maximum length of a ATT attribute value field. This is determined by the max packet
// size (255) less the minimum number of bytes used by other fields in any packet.
const MAX_ATTRIBUTE_LEN: usize = 248;

impl Debug for GattAttributeModified {
    #[cfg(feature = "ms")]
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "{{conn_handle: {:?}, attr_handle: {:?}, offset: {}, continued: {}, data: {:?}}}",
            self.conn_handle,
            self.attr_handle,
            self.offset,
            self.continued,
            first_16(self.data()),
        )
    }

    #[cfg(not(feature = "ms"))]
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "{{conn_handle: {:?}, attr_handle: {:?}, data: {:?}}}",
            self.conn_handle,
            self.attr_handle,
            first_16(self.data()),
        )
    }
}

#[cfg(feature = "ms")]
fn to_gatt_attribute_modified(
    buffer: &[u8],
) -> Result<GattAttributeModified, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 9);

    let data_len = buffer[6] as usize;
    require_len!(buffer, 9 + data_len);

    let mut data = [0; MAX_ATTRIBUTE_LEN];
    data[..data_len].copy_from_slice(&buffer[9..]);

    let offset_field = LittleEndian::read_u16(&buffer[7..]);
    Ok(GattAttributeModified {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        attr_handle: AttributeHandle(LittleEndian::read_u16(&buffer[4..])),
        offset: (offset_field & 0x7FFF) as usize,
        continued: (offset_field & 0x8000) > 0,
        data_len: data_len,
        data_buf: data,
    })
}

#[cfg(not(feature = "ms"))]
fn to_gatt_attribute_modified(
    buffer: &[u8],
) -> Result<GattAttributeModified, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 7);

    let data_len = buffer[6] as usize;
    require_len!(buffer, 7 + data_len);

    let mut data = [0; MAX_ATTRIBUTE_LEN];
    data[..data_len].copy_from_slice(&buffer[7..]);

    Ok(GattAttributeModified {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        attr_handle: AttributeHandle(LittleEndian::read_u16(&buffer[4..])),
        data_len: data_len,
        data_buf: data,
    })
}

/// This event is generated in response to an Exchange MTU request.
#[derive(Copy, Clone, Debug)]
pub struct AttExchangeMtuResponse {
    ///  The connection handle related to the response.
    pub conn_handle: ConnectionHandle,

    /// Attribute server receive MTU size.
    pub server_rx_mtu: usize,
}

fn to_att_exchange_mtu_resp(
    buffer: &[u8],
) -> Result<AttExchangeMtuResponse, hci::event::Error<Error>> {
    require_len!(buffer, 7);
    Ok(AttExchangeMtuResponse {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        server_rx_mtu: LittleEndian::read_u16(&buffer[5..]) as usize,
    })
}

/// This event is generated in response to a Find Information Request. See Find Information Response
/// in Bluetooth Core v4.0 spec.
#[derive(Copy, Clone, Debug)]
pub struct AttFindInformationResponse {
    /// The connection handle related to the response
    pub conn_handle: ConnectionHandle,
    /// The Find Information Response shall have complete handle-UUID pairs. Such pairs shall not be
    /// split across response packets; this also implies that a handleUUID pair shall fit into a
    /// single response packet. The handle-UUID pairs shall be returned in ascending order of
    /// attribute handles.
    handle_uuid_pairs: HandleUuidPairs,
}

impl AttFindInformationResponse {
    /// The Find Information Response shall have complete handle-UUID pairs. Such pairs shall not be
    /// split across response packets; this also implies that a handleUUID pair shall fit into a
    /// single response packet. The handle-UUID pairs shall be returned in ascending order of
    /// attribute handles.
    pub fn handle_uuid_pair_iter<'a>(&'a self) -> HandleUuidPairIterator<'a> {
        match self.handle_uuid_pairs {
            HandleUuidPairs::Format16(count, ref data) => {
                HandleUuidPairIterator::Format16(HandleUuid16PairIterator {
                    data: data,
                    count: count,
                    next_index: 0,
                })
            }
            HandleUuidPairs::Format128(count, ref data) => {
                HandleUuidPairIterator::Format128(HandleUuid128PairIterator {
                    data: data,
                    count: count,
                    next_index: 0,
                })
            }
        }
    }
}

// Assuming a maximum HCI packet size of 255, these are the maximum number of handle-UUID pairs for
// each format that can be in one packet.  Formats cannot be mixed in a single packet.
//
// Packets have 6 other bytes of data preceding the handle-UUID pairs.
//
// max = floor((255 - 6) / pair_length)
const MAX_FORMAT16_PAIR_COUNT: usize = 62;
const MAX_FORMAT128_PAIR_COUNT: usize = 13;

/// One format of the handle-UUID pairs in the AttFindInformationResponse event. The UUIDs are
/// 16 bits.
#[derive(Copy, Clone, Debug)]
pub struct HandleUuid16Pair {
    /// Attribute handle
    pub handle: AttributeHandle,
    /// Attribute UUID
    pub uuid: Uuid16,
}

/// One format of the handle-UUID pairs in the AttFindInformationResponse event. The UUIDs are
/// 128 bits.
#[derive(Copy, Clone, Debug)]
pub struct HandleUuid128Pair {
    /// Attribute handle
    pub handle: AttributeHandle,
    /// Attribute UUID
    pub uuid: Uuid128,
}

/// Newtype for the 16-bit UUID buffer.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Uuid16(pub u16);

/// Newtype for the 128-bit UUID buffer.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Uuid128(pub [u8; 16]);

#[derive(Copy, Clone)]
enum HandleUuidPairs {
    Format16(usize, [HandleUuid16Pair; MAX_FORMAT16_PAIR_COUNT]),
    Format128(usize, [HandleUuid128Pair; MAX_FORMAT128_PAIR_COUNT]),
}

impl Debug for HandleUuidPairs {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{{")?;
        match *self {
            HandleUuidPairs::Format16(count, pairs) => {
                for handle_uuid_pair in &pairs[..count] {
                    write!(
                        f,
                        "{{{:?}, {:?}}}",
                        handle_uuid_pair.handle, handle_uuid_pair.uuid
                    )?
                }
            }
            HandleUuidPairs::Format128(count, pairs) => {
                for handle_uuid_pair in &pairs[..count] {
                    write!(
                        f,
                        "{{{:?}, {:?}}}",
                        handle_uuid_pair.handle, handle_uuid_pair.uuid
                    )?
                }
            }
        }
        write!(f, "}}")
    }
}

/// Possible iterators over handle-UUID pairs that can be returnedby the ATT find information
/// response. All pairs from the same event have the same format.
pub enum HandleUuidPairIterator<'a> {
    /// The event contains 16-bit UUIDs.
    Format16(HandleUuid16PairIterator<'a>),
    /// The event contains 128-bit UUIDs.
    Format128(HandleUuid128PairIterator<'a>),
}

/// Iterator over handle-UUID pairs for 16-bit UUIDs.
pub struct HandleUuid16PairIterator<'a> {
    data: &'a [HandleUuid16Pair; MAX_FORMAT16_PAIR_COUNT],
    count: usize,
    next_index: usize,
}

impl<'a> Iterator for HandleUuid16PairIterator<'a> {
    type Item = HandleUuid16Pair;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.count {
            return None;
        }

        let index = self.next_index;
        self.next_index += 1;
        Some(self.data[index])
    }
}

/// Iterator over handle-UUID pairs for 128-bit UUIDs.
pub struct HandleUuid128PairIterator<'a> {
    data: &'a [HandleUuid128Pair; MAX_FORMAT128_PAIR_COUNT],
    count: usize,
    next_index: usize,
}

impl<'a> Iterator for HandleUuid128PairIterator<'a> {
    type Item = HandleUuid128Pair;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.count {
            return None;
        }

        let index = self.next_index;
        self.next_index += 1;
        Some(self.data[index])
    }
}

fn to_att_find_information_response(
    buffer: &[u8],
) -> Result<AttFindInformationResponse, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 6);

    let data_len = buffer[4] as usize;
    require_len!(buffer, 5 + data_len);

    Ok(AttFindInformationResponse {
        conn_handle: to_conn_handle(buffer)?,
        handle_uuid_pairs: match buffer[5] {
            1 => to_handle_uuid16_pairs(&buffer[6..]).map_err(hci::event::Error::Vendor)?,
            2 => to_handle_uuid128_pairs(&buffer[6..]).map_err(hci::event::Error::Vendor)?,
            _ => {
                return Err(hci::event::Error::Vendor(
                    Error::BadAttFindInformationResponseFormat(buffer[5]),
                ));
            }
        },
    })
}

fn to_handle_uuid16_pairs(buffer: &[u8]) -> Result<HandleUuidPairs, Error> {
    const PAIR_LEN: usize = 4;
    if buffer.len() % PAIR_LEN != 0 {
        return Err(Error::AttFindInformationResponsePartialPair16);
    }

    let count = buffer.len() / PAIR_LEN;
    let mut pairs = [HandleUuid16Pair {
        handle: AttributeHandle(0),
        uuid: Uuid16(0),
    }; MAX_FORMAT16_PAIR_COUNT];
    for i in 0..count {
        let index = i * PAIR_LEN;
        pairs[i].handle = AttributeHandle(LittleEndian::read_u16(&buffer[index..]));
        pairs[i].uuid = Uuid16(LittleEndian::read_u16(&buffer[2 + index..]));
    }

    Ok(HandleUuidPairs::Format16(count, pairs))
}

fn to_handle_uuid128_pairs(buffer: &[u8]) -> Result<HandleUuidPairs, Error> {
    const PAIR_LEN: usize = 18;
    if buffer.len() % PAIR_LEN != 0 {
        return Err(Error::AttFindInformationResponsePartialPair128);
    }

    let count = buffer.len() / PAIR_LEN;
    let mut pairs = [HandleUuid128Pair {
        handle: AttributeHandle(0),
        uuid: Uuid128([0; 16]),
    }; MAX_FORMAT128_PAIR_COUNT];
    for i in 0..count {
        let index = i * PAIR_LEN;
        let next_index = (i + 1) * PAIR_LEN;
        pairs[i].handle = AttributeHandle(LittleEndian::read_u16(&buffer[index..]));
        pairs[i]
            .uuid
            .0
            .copy_from_slice(&buffer[2 + index..next_index]);
    }

    Ok(HandleUuidPairs::Format128(count, pairs))
}

/// This event is generated in response to a Find By Type Value Request.
#[derive(Copy, Clone)]
pub struct AttFindByTypeValueResponse {
    /// The connection handle related to the response.
    pub conn_handle: ConnectionHandle,

    /// The number of valid pairs that follow.
    handle_pair_count: usize,

    /// Handles Information List as defined in Bluetooth Core v4.1 spec.
    handles: [HandleInfoPair; MAX_HANDLE_INFO_PAIR_COUNT],
}

impl AttFindByTypeValueResponse {
    /// Returns an iterator over the Handles Information List as defined in Bluetooth Core v4.1
    /// spec.
    pub fn handle_pairs_iter<'a>(&'a self) -> HandleInfoPairIterator<'a> {
        HandleInfoPairIterator {
            event: &self,
            next_index: 0,
        }
    }
}

impl Debug for AttFindByTypeValueResponse {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{{.conn_handle = {:?}, ", self.conn_handle)?;
        for handle_pair in self.handle_pairs_iter() {
            write!(f, "{:?}", handle_pair)?;
        }
        write!(f, "}}")
    }
}

// Assuming a maximum HCI packet size of 255, these are the maximum number of handle pairs that can
// be in one packet.
//
// Packets have 5 other bytes of data preceding the handle-UUID pairs.
//
// max = floor((255 - 5) / 4)
const MAX_HANDLE_INFO_PAIR_COUNT: usize = 62;

/// Simple container for the handle information returned in AttFindByTypeValueResponse.
#[derive(Copy, Clone, Debug)]
pub struct HandleInfoPair {
    /// Attribute handle
    pub attribute: AttributeHandle,
    /// Group End handle
    pub group_end: GroupEndHandle,
}

/// Newtype for Group End handles
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GroupEndHandle(pub u16);

/// Iterator into valid HandleInfoPair structs returned in the ATT Find By Type Value Response
/// event.
pub struct HandleInfoPairIterator<'a> {
    event: &'a AttFindByTypeValueResponse,
    next_index: usize,
}

impl<'a> Iterator for HandleInfoPairIterator<'a> {
    type Item = HandleInfoPair;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.event.handle_pair_count {
            return None;
        }

        let index = self.next_index;
        self.next_index += 1;
        Some(self.event.handles[index])
    }
}

fn to_att_find_by_value_type_response(
    buffer: &[u8],
) -> Result<AttFindByTypeValueResponse, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 5);

    let data_len = buffer[4] as usize;
    require_len!(buffer, 5 + data_len);

    const PAIR_LEN: usize = 4;
    let pair_buffer = &buffer[5..];
    if pair_buffer.len() % PAIR_LEN != 0 {
        return Err(hci::event::Error::Vendor(Error::AttFindByTypeValuePartial));
    }

    let count = pair_buffer.len() / PAIR_LEN;
    let mut pairs = [HandleInfoPair {
        attribute: AttributeHandle(0),
        group_end: GroupEndHandle(0),
    }; MAX_HANDLE_INFO_PAIR_COUNT];
    for i in 0..count {
        let index = i * PAIR_LEN;
        pairs[i].attribute = AttributeHandle(LittleEndian::read_u16(&pair_buffer[index..]));
        pairs[i].group_end = GroupEndHandle(LittleEndian::read_u16(&pair_buffer[2 + index..]));
    }
    Ok(AttFindByTypeValueResponse {
        conn_handle: to_conn_handle(buffer)?,
        handle_pair_count: count,
        handles: pairs,
    })
}

/// This event is generated in response to a Read By Type Request.
#[derive(Copy, Clone)]
pub struct AttReadByTypeResponse {
    /// The connection handle related to the response.
    pub conn_handle: ConnectionHandle,

    /// Number of valid bytes in handle_value_pair_buf
    data_len: usize,
    /// Length of each value in handle_value_pair_buf
    value_len: usize,
    /// Raw data of the response. Contains 2 octets for the attribute handle followed by |value_len|
    /// octets of value data. These pairs repeat for |data_len| bytes.
    handle_value_pair_buf: [u8; MAX_HANDLE_VALUE_PAIR_BUF_LEN],
}

// The maximum amount of data in the buffer is the max HCI packet size (255) less the other data in
// the packet.
const MAX_HANDLE_VALUE_PAIR_BUF_LEN: usize = 249;

impl Debug for AttReadByTypeResponse {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{{.conn_handle = {:?}, ", self.conn_handle)?;
        for handle_value_pair in self.handle_value_pair_iter() {
            write!(
                f,
                "{{handle: {:?}, value: {:?}}}",
                handle_value_pair.handle,
                first_16(handle_value_pair.value)
            )?;
        }
        write!(f, "}}")
    }
}

impl AttReadByTypeResponse {
    /// Return an iterator over all valid handle-value pairs returned with the ATT Read by Type
    /// response.
    pub fn handle_value_pair_iter<'a>(&'a self) -> HandleValuePairIterator<'a> {
        HandleValuePairIterator {
            event: &self,
            index: 0,
        }
    }
}

/// Iterator over the valid handle-value pairs returned with the ATT Read by Type response.
pub struct HandleValuePairIterator<'a> {
    event: &'a AttReadByTypeResponse,
    index: usize,
}

impl<'a> Iterator for HandleValuePairIterator<'a> {
    type Item = HandleValuePair<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.event.data_len {
            return None;
        }

        let handle_index = self.index;
        let value_index = self.index + 2;
        self.index += 2 + self.event.value_len;
        let next_index = self.index;
        Some(HandleValuePair {
            handle: AttributeHandle(LittleEndian::read_u16(
                &self.event.handle_value_pair_buf[handle_index..],
            )),
            value: &self.event.handle_value_pair_buf[value_index..next_index],
        })
    }
}

/// A single handle-value pair returned by the ATT Read by Type response.
pub struct HandleValuePair<'a> {
    /// Attribute handle
    pub handle: AttributeHandle,
    /// Attribute value. The caller must interpret the value correctly, depending on the expected
    /// type of the attribute.
    pub value: &'a [u8],
}

fn to_att_read_by_type_response(
    buffer: &[u8],
) -> Result<AttReadByTypeResponse, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 6);

    let data_len = buffer[4] as usize;
    require_len!(buffer, 5 + data_len);

    let handle_value_pair_len = buffer[5] as usize;
    let handle_value_pair_buf = &buffer[6..];
    if handle_value_pair_buf.len() % handle_value_pair_len != 0 {
        return Err(hci::event::Error::Vendor(
            Error::AttReadByTypeResponsePartial,
        ));
    }

    let mut full_handle_value_pair_buf = [0; MAX_HANDLE_VALUE_PAIR_BUF_LEN];
    full_handle_value_pair_buf[..handle_value_pair_buf.len()]
        .copy_from_slice(&handle_value_pair_buf);

    Ok(AttReadByTypeResponse {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        data_len: handle_value_pair_buf.len(),
        value_len: handle_value_pair_len - 2,
        handle_value_pair_buf: full_handle_value_pair_buf,
    })
}

/// This event is generated in response to a Read Request.
#[derive(Copy, Clone)]
pub struct AttReadResponse {
    /// The connection handle related to the response.
    pub conn_handle: ConnectionHandle,

    /// The number of valid bytes in the value buffer.
    value_len: usize,

    /// Buffer containing the value data.
    value_buf: [u8; MAX_READ_RESPONSE_LEN],
}

// The maximum amount of data in the buffer is the max HCI packet size (255) less the other data in
// the packet.
const MAX_READ_RESPONSE_LEN: usize = 250;

impl Debug for AttReadResponse {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "{{.conn_handle = {:?}, value = {:?}}}",
            self.conn_handle,
            first_16(self.value())
        )
    }
}

impl AttReadResponse {
    /// Returns the valid part of the value data.
    pub fn value(&self) -> &[u8] {
        &self.value_buf[..self.value_len]
    }
}

fn to_att_read_response(buffer: &[u8]) -> Result<AttReadResponse, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 5);

    let data_len = buffer[4] as usize;
    require_len!(buffer, 5 + data_len);

    let mut value_buf = [0; MAX_READ_RESPONSE_LEN];
    value_buf[..data_len].copy_from_slice(&buffer[5..]);

    Ok(AttReadResponse {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        value_len: data_len,
        value_buf: value_buf,
    })
}

/// This event is generated in response to a Read By Group Type Request. See the Bluetooth Core v4.1
/// spec, Vol 3, section 3.4.4.9 and 3.4.4.10.
#[derive(Copy, Clone)]
pub struct AttReadByGroupTypeResponse {
    ///  The connection handle related to the response.
    pub conn_handle: ConnectionHandle,

    /// Number of valid bytes in attribute_data_buf
    data_len: usize,

    /// Length of the attribute data group in attribute_data_buf, including the attribute and group
    /// end handles.
    attribute_group_len: usize,

    /// List of attribute data which is a repetition of:
    /// 1. 2 octets for attribute handle
    /// 2. 2 octets for end group handle
    /// 3. (attribute_group_len - 4) octets for attribute value
    attribute_data_buf: [u8; MAX_ATTRIBUTE_DATA_BUF_LEN],
}

// The maximum amount of data in the buffer is the max HCI packet size (255) less the other data in
// the packet.
const MAX_ATTRIBUTE_DATA_BUF_LEN: usize = 249;

impl AttReadByGroupTypeResponse {
    /// Create and return an iterator for the attribute data returned with the response.
    pub fn attribute_data_iter<'a>(&'a self) -> AttributeDataIterator<'a> {
        AttributeDataIterator {
            event: self,
            next_index: 0,
        }
    }
}

impl Debug for AttReadByGroupTypeResponse {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{{.conn_handle = {:?}, ", self.conn_handle)?;
        for attribute_data in self.attribute_data_iter() {
            write!(
                f,
                "{{.attribute_handle = {:?}, .group_end_handle = {:?}, .value = {:?}}}",
                attribute_data.attribute_handle,
                attribute_data.group_end_handle,
                first_16(attribute_data.value)
            )?;
        }
        write!(f, "}}")
    }
}

/// Iterator over the attribute data returned in the AttReadByGroupTypeResponse.
pub struct AttributeDataIterator<'a> {
    event: &'a AttReadByGroupTypeResponse,
    next_index: usize,
}

impl<'a> Iterator for AttributeDataIterator<'a> {
    type Item = AttributeData<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.event.data_len {
            return None;
        }

        let attr_handle_index = self.next_index;
        let group_end_index = 2 + attr_handle_index;
        let value_index = 2 + group_end_index;
        self.next_index += self.event.attribute_group_len;
        Some(AttributeData {
            attribute_handle: AttributeHandle(LittleEndian::read_u16(
                &self.event.attribute_data_buf[attr_handle_index..],
            )),
            group_end_handle: GroupEndHandle(LittleEndian::read_u16(
                &self.event.attribute_data_buf[group_end_index..],
            )),
            value: &self.event.attribute_data_buf[value_index..self.next_index],
        })
    }
}

/// Attribute data returned in the AttReadByGroupTypeResponse event.
pub struct AttributeData<'a> {
    /// Attribute handle
    pub attribute_handle: AttributeHandle,
    /// Group end handle
    pub group_end_handle: GroupEndHandle,
    /// Attribute value
    pub value: &'a [u8],
}

fn to_att_read_by_group_type_response(
    buffer: &[u8],
) -> Result<AttReadByGroupTypeResponse, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 6);

    let data_len = buffer[4] as usize;
    require_len!(buffer, 5 + data_len);

    let attribute_group_len = buffer[5] as usize;

    if &buffer[6..].len() % attribute_group_len != 0 {
        return Err(hci::event::Error::Vendor(
            Error::AttReadByGroupTypeResponsePartial,
        ));
    }

    let mut attribute_data_buf = [0; MAX_ATTRIBUTE_DATA_BUF_LEN];
    attribute_data_buf[..data_len - 1].copy_from_slice(&buffer[6..]);
    Ok(AttReadByGroupTypeResponse {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        data_len: data_len - 1, // lose 1 byte to attribute_group_len
        attribute_group_len: attribute_group_len,
        attribute_data_buf: attribute_data_buf,
    })
}

/// This event is generated in response to a Prepare Write Request. See the Bluetooth Core v4.1
/// spec, Vol 3, Part F, section 3.4.6.1 and 3.4.6.2
#[derive(Copy, Clone)]
pub struct AttPrepareWriteResponse {
    /// The connection handle related to the response.
    pub conn_handle: ConnectionHandle,
    /// The handle of the attribute to be written.
    pub attribute_handle: AttributeHandle,
    /// The offset of the first octet to be written.
    pub offset: usize,

    /// Number of valid bytes in |value_buf|
    value_len: usize,
    value_buf: [u8; MAX_WRITE_RESPONSE_VALUE_LEN],
}

// The maximum amount of data in the buffer is the max HCI packet size (255) less the other data in
// the packet.
const MAX_WRITE_RESPONSE_VALUE_LEN: usize = 246;

impl Debug for AttPrepareWriteResponse {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "{{.conn_handle = {:?}, .attribute_handle = {:?}, .offset = {}, .value = {:?}}}",
            self.conn_handle,
            self.attribute_handle,
            self.offset,
            first_16(self.value())
        )
    }
}

impl AttPrepareWriteResponse {
    /// Returns the partial value of the attribute to be written.
    pub fn value(&self) -> &[u8] {
        &self.value_buf[..self.value_len]
    }
}

fn to_att_prepare_write_response(
    buffer: &[u8],
) -> Result<AttPrepareWriteResponse, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 9);

    let data_len = buffer[4] as usize;
    require_len!(buffer, 5 + data_len);

    let value_len = data_len - 4;
    let mut value_buf = [0; MAX_WRITE_RESPONSE_VALUE_LEN];
    value_buf[..value_len].copy_from_slice(&buffer[9..]);
    Ok(AttPrepareWriteResponse {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        attribute_handle: AttributeHandle(LittleEndian::read_u16(&buffer[5..])),
        offset: LittleEndian::read_u16(&buffer[7..]) as usize,
        value_len: value_len,
        value_buf: value_buf,
    })
}

/// Defines the attribute value returned by a GATT Indication or GATT Notification event.
#[derive(Copy, Clone)]
pub struct AttributeValue {
    /// The connection handle related to the event
    pub conn_handle: ConnectionHandle,
    /// The handle of the attribute
    pub attribute_handle: AttributeHandle,

    /// Number of valid bytes in value_buf
    value_len: usize,
    /// Current value of the attribute. Only the first value_len bytes are valid.
    value_buf: [u8; MAX_ATTRIBUTE_VALUE_LEN],
}

// The maximum amount of data in the buffer is the max HCI packet size (255) less the other data in
// the packet.
const MAX_ATTRIBUTE_VALUE_LEN: usize = 248;

impl Debug for AttributeValue {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "{{.conn_handle = {:?}, .attribute_handle = {:?}, .value = {:?}}}",
            self.conn_handle,
            self.attribute_handle,
            first_16(self.value())
        )
    }
}

impl AttributeValue {
    /// Returns the current value of the attribute.
    pub fn value(&self) -> &[u8] {
        &self.value_buf[..self.value_len]
    }
}

fn to_attribute_value(buffer: &[u8]) -> Result<AttributeValue, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 7);

    let data_len = buffer[4] as usize;
    require_len!(buffer, 5 + data_len);

    let value_len = data_len - 2;
    let mut value_buf = [0; MAX_ATTRIBUTE_VALUE_LEN];
    value_buf[..value_len].copy_from_slice(&buffer[7..]);
    Ok(AttributeValue {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        attribute_handle: AttributeHandle(LittleEndian::read_u16(&buffer[5..])),
        value_len: value_len,
        value_buf: value_buf,
    })
}

fn to_write_permit_request(buffer: &[u8]) -> Result<AttributeValue, hci::event::Error<Error>> {
    require_len_at_least!(buffer, 7);

    let data_len = buffer[6] as usize;
    require_len!(buffer, 7 + data_len);

    let value_len = data_len;
    let mut value_buf = [0; MAX_ATTRIBUTE_VALUE_LEN];
    value_buf[..value_len].copy_from_slice(&buffer[7..]);
    Ok(AttributeValue {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        attribute_handle: AttributeHandle(LittleEndian::read_u16(&buffer[4..])),
        value_len: value_len,
        value_buf: value_buf,
    })
}

/// This event is generated when a GATT client procedure completes either with error or
/// successfully.
#[derive(Copy, Clone, Debug)]
pub struct GattProcedureComplete {
    /// The connection handle for which the gatt procedure has completed
    pub conn_handle: ConnectionHandle,

    /// Indicates whether the procedure completed with error (GattProcedureStatus::Failed) or was
    /// successful (GattProcedureStatus::Success).
    pub status: GattProcedureStatus,
}

/// Allowed status codes for the GATT Procedure Complete event.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GattProcedureStatus {
    /// BLE Status Success
    Success,
    /// BLE Status Failed
    Failed,
}

impl TryFrom<u8> for GattProcedureStatus {
    type Error = Error;

    fn try_from(value: u8) -> Result<GattProcedureStatus, Self::Error> {
        match value {
            0x00 => Ok(GattProcedureStatus::Success),
            0x41 => Ok(GattProcedureStatus::Failed),
            _ => Err(Error::BadGattProcedureStatus(value)),
        }
    }
}

fn to_gatt_procedure_complete(
    buffer: &[u8],
) -> Result<GattProcedureComplete, hci::event::Error<Error>> {
    require_len!(buffer, 6);

    Ok(GattProcedureComplete {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        status: buffer[5].try_into().map_err(hci::event::Error::Vendor)?,
    })
}

/// The Error Response is used to state that a given request cannot be performed, and to provide the
/// reason. See the Bluetooth Core Specification, v4.1, Vol 3, Part F, Section 3.4.1.1.
#[derive(Copy, Clone, Debug)]
pub struct AttErrorResponse {
    /// The connection handle related to the event.
    pub conn_handle: ConnectionHandle,
    /// The request that generated this error response.
    pub request: AttRequest,
    ///The attribute handle that generated this error response.
    pub attribute_handle: AttributeHandle,
    /// The reason why the request has generated an error response.
    pub error: AttError,
}

/// Potential error codes for the ATT Error Response. See Table 3.3 in the Bluetooth Core
/// Specification, v4.1, Vol 3, PartF, Section 3.4.1.1 and The Bluetooth Core Specification
/// Supplement, Table 1.1.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AttError {
    /// The attribute handle given was not valid on this server.
    InvalidHandle = 0x01,
    /// The attribute cannot be read.
    ReadNotPermitted = 0x02,
    /// The attribute cannot be written.
    WriteNotPermitted = 0x03,
    /// The attribute PDU was invalid.
    InvalidPdu = 0x04,
    /// The attribute requires authentication before it can be read or written.
    InsufficientAuthentication = 0x05,
    /// Attribute server does not support the request received from the client.
    RequestNotSupported = 0x06,
    /// Offset specified was past the end of the attribute.
    InvalidOffset = 0x07,
    /// The attribute requires authorization before it can be read or written.
    InsufficientAuthorization = 0x08,
    /// Too many prepare writes have been queued.
    PrepareQueueFull = 0x09,
    /// No attribute found within the given attribute handle range.
    AttributeNotFound = 0x0A,
    /// The attribute cannot be read or written using the Read Blob Request.
    AttributeNotLong = 0x0B,
    /// The Encryption Key Size used for encrypting this link is insufficient.
    InsufficientEncryptionKeySize = 0x0C,
    /// The attribute value length is invalid for the operation.
    InvalidAttributeValueLength = 0x0D,
    /// The attribute request that was requested has encountered an error that was unlikely, and
    /// therefore could not be completed as requested.
    UnlikelyError = 0x0E,
    /// The attribute requires encryption before it can be read or written.
    InsufficientEncryption = 0x0F,
    /// The attribute type is not a supported grouping attribute as defined by a higher layer
    /// specification.
    UnsupportedGroupType = 0x10,
    /// Insufficient Resources to complete the request.
    InsufficientResources = 0x11,
    /// Reserved for future use
    Reserved0x12 = 0x12,
    /// Reserved for future use
    Reserved0x13 = 0x13,
    /// Reserved for future use
    Reserved0x14 = 0x14,
    /// Reserved for future use
    Reserved0x15 = 0x15,
    /// Reserved for future use
    Reserved0x16 = 0x16,
    /// Reserved for future use
    Reserved0x17 = 0x17,
    /// Reserved for future use
    Reserved0x18 = 0x18,
    /// Reserved for future use
    Reserved0x19 = 0x19,
    /// Reserved for future use
    Reserved0x1A = 0x1A,
    /// Reserved for future use
    Reserved0x1B = 0x1B,
    /// Reserved for future use
    Reserved0x1C = 0x1C,
    /// Reserved for future use
    Reserved0x1D = 0x1D,
    /// Reserved for future use
    Reserved0x1E = 0x1E,
    /// Reserved for future use
    Reserved0x1F = 0x1F,
    /// Reserved for future use
    Reserved0x20 = 0x20,
    /// Reserved for future use
    Reserved0x21 = 0x21,
    /// Reserved for future use
    Reserved0x22 = 0x22,
    /// Reserved for future use
    Reserved0x23 = 0x23,
    /// Reserved for future use
    Reserved0x24 = 0x24,
    /// Reserved for future use
    Reserved0x25 = 0x25,
    /// Reserved for future use
    Reserved0x26 = 0x26,
    /// Reserved for future use
    Reserved0x27 = 0x27,
    /// Reserved for future use
    Reserved0x28 = 0x28,
    /// Reserved for future use
    Reserved0x29 = 0x29,
    /// Reserved for future use
    Reserved0x2A = 0x2A,
    /// Reserved for future use
    Reserved0x2B = 0x2B,
    /// Reserved for future use
    Reserved0x2C = 0x2C,
    /// Reserved for future use
    Reserved0x2D = 0x2D,
    /// Reserved for future use
    Reserved0x2E = 0x2E,
    /// Reserved for future use
    Reserved0x2F = 0x2F,
    /// Reserved for future use
    Reserved0x30 = 0x30,
    /// Reserved for future use
    Reserved0x31 = 0x31,
    /// Reserved for future use
    Reserved0x32 = 0x32,
    /// Reserved for future use
    Reserved0x33 = 0x33,
    /// Reserved for future use
    Reserved0x34 = 0x34,
    /// Reserved for future use
    Reserved0x35 = 0x35,
    /// Reserved for future use
    Reserved0x36 = 0x36,
    /// Reserved for future use
    Reserved0x37 = 0x37,
    /// Reserved for future use
    Reserved0x38 = 0x38,
    /// Reserved for future use
    Reserved0x39 = 0x39,
    /// Reserved for future use
    Reserved0x3A = 0x3A,
    /// Reserved for future use
    Reserved0x3B = 0x3B,
    /// Reserved for future use
    Reserved0x3C = 0x3C,
    /// Reserved for future use
    Reserved0x3D = 0x3D,
    /// Reserved for future use
    Reserved0x3E = 0x3E,
    /// Reserved for future use
    Reserved0x3F = 0x3F,
    /// Reserved for future use
    Reserved0x40 = 0x40,
    /// Reserved for future use
    Reserved0x41 = 0x41,
    /// Reserved for future use
    Reserved0x42 = 0x42,
    /// Reserved for future use
    Reserved0x43 = 0x43,
    /// Reserved for future use
    Reserved0x44 = 0x44,
    /// Reserved for future use
    Reserved0x45 = 0x45,
    /// Reserved for future use
    Reserved0x46 = 0x46,
    /// Reserved for future use
    Reserved0x47 = 0x47,
    /// Reserved for future use
    Reserved0x48 = 0x48,
    /// Reserved for future use
    Reserved0x49 = 0x49,
    /// Reserved for future use
    Reserved0x4A = 0x4A,
    /// Reserved for future use
    Reserved0x4B = 0x4B,
    /// Reserved for future use
    Reserved0x4C = 0x4C,
    /// Reserved for future use
    Reserved0x4D = 0x4D,
    /// Reserved for future use
    Reserved0x4E = 0x4E,
    /// Reserved for future use
    Reserved0x4F = 0x4F,
    /// Reserved for future use
    Reserved0x50 = 0x50,
    /// Reserved for future use
    Reserved0x51 = 0x51,
    /// Reserved for future use
    Reserved0x52 = 0x52,
    /// Reserved for future use
    Reserved0x53 = 0x53,
    /// Reserved for future use
    Reserved0x54 = 0x54,
    /// Reserved for future use
    Reserved0x55 = 0x55,
    /// Reserved for future use
    Reserved0x56 = 0x56,
    /// Reserved for future use
    Reserved0x57 = 0x57,
    /// Reserved for future use
    Reserved0x58 = 0x58,
    /// Reserved for future use
    Reserved0x59 = 0x59,
    /// Reserved for future use
    Reserved0x5A = 0x5A,
    /// Reserved for future use
    Reserved0x5B = 0x5B,
    /// Reserved for future use
    Reserved0x5C = 0x5C,
    /// Reserved for future use
    Reserved0x5D = 0x5D,
    /// Reserved for future use
    Reserved0x5E = 0x5E,
    /// Reserved for future use
    Reserved0x5F = 0x5F,
    /// Reserved for future use
    Reserved0x60 = 0x60,
    /// Reserved for future use
    Reserved0x61 = 0x61,
    /// Reserved for future use
    Reserved0x62 = 0x62,
    /// Reserved for future use
    Reserved0x63 = 0x63,
    /// Reserved for future use
    Reserved0x64 = 0x64,
    /// Reserved for future use
    Reserved0x65 = 0x65,
    /// Reserved for future use
    Reserved0x66 = 0x66,
    /// Reserved for future use
    Reserved0x67 = 0x67,
    /// Reserved for future use
    Reserved0x68 = 0x68,
    /// Reserved for future use
    Reserved0x69 = 0x69,
    /// Reserved for future use
    Reserved0x6A = 0x6A,
    /// Reserved for future use
    Reserved0x6B = 0x6B,
    /// Reserved for future use
    Reserved0x6C = 0x6C,
    /// Reserved for future use
    Reserved0x6D = 0x6D,
    /// Reserved for future use
    Reserved0x6E = 0x6E,
    /// Reserved for future use
    Reserved0x6F = 0x6F,
    /// Reserved for future use
    Reserved0x70 = 0x70,
    /// Reserved for future use
    Reserved0x71 = 0x71,
    /// Reserved for future use
    Reserved0x72 = 0x72,
    /// Reserved for future use
    Reserved0x73 = 0x73,
    /// Reserved for future use
    Reserved0x74 = 0x74,
    /// Reserved for future use
    Reserved0x75 = 0x75,
    /// Reserved for future use
    Reserved0x76 = 0x76,
    /// Reserved for future use
    Reserved0x77 = 0x77,
    /// Reserved for future use
    Reserved0x78 = 0x78,
    /// Reserved for future use
    Reserved0x79 = 0x79,
    /// Reserved for future use
    Reserved0x7A = 0x7A,
    /// Reserved for future use
    Reserved0x7B = 0x7B,
    /// Reserved for future use
    Reserved0x7C = 0x7C,
    /// Reserved for future use
    Reserved0x7D = 0x7D,
    /// Reserved for future use
    Reserved0x7E = 0x7E,
    /// Reserved for future use
    Reserved0x7F = 0x7F,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x80 = 0x80,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x81 = 0x81,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x82 = 0x82,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x83 = 0x83,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x84 = 0x84,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x85 = 0x85,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x86 = 0x86,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x87 = 0x87,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x88 = 0x88,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x89 = 0x89,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8A = 0x8A,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8B = 0x8B,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8C = 0x8C,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8D = 0x8D,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8E = 0x8E,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8F = 0x8F,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x90 = 0x90,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x91 = 0x91,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x92 = 0x92,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x93 = 0x93,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x94 = 0x94,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x95 = 0x95,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x96 = 0x96,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x97 = 0x97,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x98 = 0x98,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x99 = 0x99,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9A = 0x9A,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9B = 0x9B,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9C = 0x9C,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9D = 0x9D,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9E = 0x9E,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9F = 0x9F,
    /// Reserved0x80 future use
    Reserveda0h = 0xA0,
    /// Reserved for future use
    Reserved0xA1 = 0xA1,
    /// Reserved for future use
    Reserved0xA2 = 0xA2,
    /// Reserved for future use
    Reserved0xA3 = 0xA3,
    /// Reserved for future use
    Reserved0xA4 = 0xA4,
    /// Reserved for future use
    Reserved0xA5 = 0xA5,
    /// Reserved for future use
    Reserved0xA6 = 0xA6,
    /// Reserved for future use
    Reserved0xA7 = 0xA7,
    /// Reserved for future use
    Reserved0xA8 = 0xA8,
    /// Reserved for future use
    Reserved0xA9 = 0xA9,
    /// Reserved for future use
    Reserved0xAA = 0xAA,
    /// Reserved for future use
    Reserved0xAB = 0xAB,
    /// Reserved for future use
    Reserved0xAC = 0xAC,
    /// Reserved for future use
    Reserved0xAD = 0xAD,
    /// Reserved for future use
    Reserved0xAE = 0xAE,
    /// Reserved for future use
    Reserved0xAF = 0xAF,
    /// Reserved for future use
    Reserved0xB0 = 0xB0,
    /// Reserved for future use
    Reserved0xB1 = 0xB1,
    /// Reserved for future use
    Reserved0xB2 = 0xB2,
    /// Reserved for future use
    Reserved0xB3 = 0xB3,
    /// Reserved for future use
    Reserved0xB4 = 0xB4,
    /// Reserved for future use
    Reserved0xB5 = 0xB5,
    /// Reserved for future use
    Reserved0xB6 = 0xB6,
    /// Reserved for future use
    Reserved0xB7 = 0xB7,
    /// Reserved for future use
    Reserved0xB8 = 0xB8,
    /// Reserved for future use
    Reserved0xB9 = 0xB9,
    /// Reserved for future use
    Reserved0xBA = 0xBA,
    /// Reserved for future use
    Reserved0xBB = 0xBB,
    /// Reserved for future use
    Reserved0xBC = 0xBC,
    /// Reserved for future use
    Reserved0xBD = 0xBD,
    /// Reserved for future use
    Reserved0xBE = 0xBE,
    /// Reserved for future use
    Reserved0xBF = 0xBF,
    /// Reserved for future use
    Reserved0xC0 = 0xC0,
    /// Reserved for future use
    Reserved0xC1 = 0xC1,
    /// Reserved for future use
    Reserved0xC2 = 0xC2,
    /// Reserved for future use
    Reserved0xC3 = 0xC3,
    /// Reserved for future use
    Reserved0xC4 = 0xC4,
    /// Reserved for future use
    Reserved0xC5 = 0xC5,
    /// Reserved for future use
    Reserved0xC6 = 0xC6,
    /// Reserved for future use
    Reserved0xC7 = 0xC7,
    /// Reserved for future use
    Reserved0xC8 = 0xC8,
    /// Reserved for future use
    Reserved0xC9 = 0xC9,
    /// Reserved for future use
    Reserved0xCA = 0xCA,
    /// Reserved for future use
    Reserved0xCB = 0xCB,
    /// Reserved for future use
    Reserved0xCC = 0xCC,
    /// Reserved for future use
    Reserved0xCD = 0xCD,
    /// Reserved for future use
    Reserved0xCE = 0xCE,
    /// Reserved for future use
    Reserved0xCF = 0xCF,
    /// Reserved for future use
    Reserved0xD0 = 0xD0,
    /// Reserved for future use
    Reserved0xD1 = 0xD1,
    /// Reserved for future use
    Reserved0xD2 = 0xD2,
    /// Reserved for future use
    Reserved0xD3 = 0xD3,
    /// Reserved for future use
    Reserved0xD4 = 0xD4,
    /// Reserved for future use
    Reserved0xD5 = 0xD5,
    /// Reserved for future use
    Reserved0xD6 = 0xD6,
    /// Reserved for future use
    Reserved0xD7 = 0xD7,
    /// Reserved for future use
    Reserved0xD8 = 0xD8,
    /// Reserved for future use
    Reserved0xD9 = 0xD9,
    /// Reserved for future use
    Reserved0xDA = 0xDA,
    /// Reserved for future use
    Reserved0xDB = 0xDB,
    /// Reserved for future use
    Reserved0xDC = 0xDC,
    /// Reserved for future use
    Reserved0xDD = 0xDD,
    /// Reserved for future use
    Reserved0xDE = 0xDE,
    /// Reserved for future use
    Reserved0xDF = 0xDF,
    /// Reserved for future use
    Reserved0xE0 = 0xE0,
    /// Reserved for future use
    Reserved0xE1 = 0xE1,
    /// Reserved for future use
    Reserved0xE2 = 0xE2,
    /// Reserved for future use
    Reserved0xE3 = 0xE3,
    /// Reserved for future use
    Reserved0xE4 = 0xE4,
    /// Reserved for future use
    Reserved0xE5 = 0xE5,
    /// Reserved for future use
    Reserved0xE6 = 0xE6,
    /// Reserved for future use
    Reserved0xE7 = 0xE7,
    /// Reserved for future use
    Reserved0xE8 = 0xE8,
    /// Reserved for future use
    Reserved0xE9 = 0xE9,
    /// Reserved for future use
    Reserved0xEA = 0xEA,
    /// Reserved for future use
    Reserved0xEB = 0xEB,
    /// Reserved for future use
    Reserved0xEC = 0xEC,
    /// Reserved for future use
    Reserved0xED = 0xED,
    /// Reserved for future use
    Reserved0xEE = 0xEE,
    /// Reserved for future use
    Reserved0xEF = 0xEF,
    /// Reserved for future use
    Reserved0xF0 = 0xF0,
    /// Reserved for future use
    Reserved0xF1 = 0xF1,
    /// Reserved for future use
    Reserved0xF2 = 0xF2,
    /// Reserved for future use
    Reserved0xF3 = 0xF3,
    /// Reserved for future use
    Reserved0xF4 = 0xF4,
    /// Reserved for future use
    Reserved0xF5 = 0xF5,
    /// Reserved for future use
    Reserved0xF6 = 0xF6,
    /// Reserved for future use
    Reserved0xF7 = 0xF7,
    /// Reserved for future use
    Reserved0xF8 = 0xF8,
    /// Reserved for future use
    Reserved0xF9 = 0xF9,
    /// Reserved for future use
    Reserved0xFA = 0xFA,
    /// Reserved for future use
    Reserved0xFB = 0xFB,
    /// The requested write operation cannot be fulfilled for reasons other than permissions.
    WriteRequestRejected = 0xFC,
    /// A Client Characteristic Configuration descriptor is not configured according to the
    /// requirements of the profile or service.
    ClientCharacteristicConfigurationDescriptorImproperlyConfigured = 0xFD,
    /// A profile or service request cannot be serviced because an operation that has been
    /// previously triggered is still in progress.
    ProcedureAlreadyInProgress = 0xFE,
    /// An attribute value is out of range as defined by a profile or service specification.
    OutOfRange = 0xFF,
}

impl From<u8> for AttError {
    fn from(value: u8) -> AttError {
        // All possible values of u8 are defined for AttError, so we can safely transmute from u8 to
        // AttError
        unsafe { mem::transmute(value) }
    }
}

/// Possible ATT requests.  See Table 3.37 in the Bluetooth Core Spec v4.1, Vol 3, Part F, Section
/// 3.4.8.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AttRequest {
    /// Section 3.4.1.1
    ErrorResponse = 0x01,
    /// Section 3.4.2.1
    ExchangeMtuRequest = 0x02,
    /// Section 3.4.2.2
    ExchangeMtuResponse = 0x03,
    /// Section 3.4.3.1
    FindInformationRequest = 0x04,
    /// Section 3.4.3.2
    FindInformationResponse = 0x05,
    /// Section 3.4.3.3
    FindByTypeValueRequest = 0x06,
    /// Section 3.4.3.4
    FindByTypeValueResponse = 0x07,
    /// Section 3.4.4.1
    ReadByTypeRequest = 0x08,
    /// Section 3.4.4.2
    ReadByTypeResponse = 0x09,
    /// Section 3.4.4.3
    ReadRequest = 0x0A,
    /// Section 3.4.4.4
    ReadResponse = 0x0B,
    /// Section 3.4.4.5
    ReadBlobRequest = 0x0C,
    /// Section 3.4.4.6
    ReadBlobResponse = 0x0D,
    /// Section 3.4.4.7
    ReadMultipleRequest = 0x0E,
    /// Section 3.4.4.8
    ReadMultipleResponse = 0x0F,
    /// Section 3.4.4.9
    ReadByGroupTypeRequest = 0x10,
    /// Section 3.4.4.10
    ReadByGroupTypeResponse = 0x11,
    /// Section 3.4.5.1
    WriteRequest = 0x12,
    /// Section 3.4.5.2
    WriteResponse = 0x13,
    /// Section 3.4.5.3
    WriteCommand = 0x52,
    /// Section 3.4.5.4
    SignedWriteCommand = 0xD2,
    /// Section 3.4.6.1
    PrepareWriteRequest = 0x16,
    /// Section 3.4.6.2
    PrepareWriteResponse = 0x17,
    /// Section 3.4.6.3
    ExecuteWriteRequest = 0x18,
    /// Section 3.4.6.4
    ExecuteWriteResponse = 0x19,
    /// Section 3.4.7.1
    HandleValueNotification = 0x1B,
    /// Section 3.4.7.2
    HandleValueIndication = 0x1D,
    /// Section 3.4.7.3
    HandleValueConfirmation = 0x1E,
}

impl TryFrom<u8> for AttRequest {
    type Error = Error;

    fn try_from(value: u8) -> Result<AttRequest, Self::Error> {
        match value {
            0x01 => Ok(AttRequest::ErrorResponse),
            0x02 => Ok(AttRequest::ExchangeMtuRequest),
            0x03 => Ok(AttRequest::ExchangeMtuResponse),
            0x04 => Ok(AttRequest::FindInformationRequest),
            0x05 => Ok(AttRequest::FindInformationResponse),
            0x06 => Ok(AttRequest::FindByTypeValueRequest),
            0x07 => Ok(AttRequest::FindByTypeValueResponse),
            0x08 => Ok(AttRequest::ReadByTypeRequest),
            0x09 => Ok(AttRequest::ReadByTypeResponse),
            0x0A => Ok(AttRequest::ReadRequest),
            0x0B => Ok(AttRequest::ReadResponse),
            0x0C => Ok(AttRequest::ReadBlobRequest),
            0x0D => Ok(AttRequest::ReadBlobResponse),
            0x0E => Ok(AttRequest::ReadMultipleRequest),
            0x0F => Ok(AttRequest::ReadMultipleResponse),
            0x10 => Ok(AttRequest::ReadByGroupTypeRequest),
            0x11 => Ok(AttRequest::ReadByGroupTypeResponse),
            0x12 => Ok(AttRequest::WriteRequest),
            0x13 => Ok(AttRequest::WriteResponse),
            0x52 => Ok(AttRequest::WriteCommand),
            0xD2 => Ok(AttRequest::SignedWriteCommand),
            0x16 => Ok(AttRequest::PrepareWriteRequest),
            0x17 => Ok(AttRequest::PrepareWriteResponse),
            0x18 => Ok(AttRequest::ExecuteWriteRequest),
            0x19 => Ok(AttRequest::ExecuteWriteResponse),
            0x1B => Ok(AttRequest::HandleValueNotification),
            0x1D => Ok(AttRequest::HandleValueIndication),
            0x1E => Ok(AttRequest::HandleValueConfirmation),
            _ => Err(Error::BadAttRequestOpcode(value)),
        }
    }
}

fn to_att_error_response(buffer: &[u8]) -> Result<AttErrorResponse, hci::event::Error<Error>> {
    require_len!(buffer, 9);
    Ok(AttErrorResponse {
        conn_handle: ConnectionHandle(LittleEndian::read_u16(&buffer[2..])),
        request: buffer[5].try_into().map_err(hci::event::Error::Vendor)?,
        attribute_handle: AttributeHandle(LittleEndian::read_u16(&buffer[6..])),
        error: buffer[8].into(),
    })
}
