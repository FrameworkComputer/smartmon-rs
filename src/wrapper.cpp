// Thin C wrapper around libsmartmon (smartmontools as a library).
// Exposes a small C API that src/lib.rs binds to.

#include <smartmon/dev_interface.h>
#include <smartmon/atacmds.h>
#include <smartmon/knowndrives.h>
#include <smartmon/nvmecmds.h>
#include <smartmon/scsicmds.h>
#include <smartmon/utility.h>

#include <cstdio>
#include <exception>
#include <memory>

// Must match `SmartmonDisk` in lib.rs
struct smartmon_disk {
  char name[64];
  char dev_type[32];
  char protocol[8];
  char model[48];
  char serial[24];
  char firmware[16];
};

static void copy_str(char * dst, size_t dstsize, const char * src)
{
  std::snprintf(dst, dstsize, "%s", src ? src : "");
}

static bool identify(smartmon::ata_device * dev, smartmon_disk & out)
{
  smartmon::ata_identify_device id{};
  if (smartmon::ata_read_identity(dev, id) < 0)
    return false;
  copy_str(out.protocol, sizeof(out.protocol), "ATA");
  smartmon::format_char_array(out.model, sizeof(out.model), id.model);
  smartmon::format_char_array(out.serial, sizeof(out.serial), id.serial_no);
  smartmon::format_char_array(out.firmware, sizeof(out.firmware), id.fw_rev);
  return true;
}

static bool identify(smartmon::nvme_device * dev, smartmon_disk & out)
{
  smartmon::nvme_id_ctrl id_ctrl{};
  if (!smartmon::nvme_read_id_ctrl(dev, id_ctrl))
    return false;
  copy_str(out.protocol, sizeof(out.protocol), "NVMe");
  smartmon::format_char_array(out.model, sizeof(out.model), id_ctrl.mn);
  smartmon::format_char_array(out.serial, sizeof(out.serial), id_ctrl.sn);
  smartmon::format_char_array(out.firmware, sizeof(out.firmware), id_ctrl.fr);
  return true;
}

static bool identify(smartmon::scsi_device * dev, smartmon_disk & out)
{
  char inq[36]{};
  if (smartmon::scsiStdInquiry(dev, (uint8_t *)inq, sizeof(inq)))
    return false;
  char vn[8 + 1], pr[16 + 1];
  copy_str(out.protocol, sizeof(out.protocol), "SCSI");
  smartmon::format_char_array(vn, inq + 8, 8);
  smartmon::format_char_array(pr, inq + 16, 16);
  std::snprintf(out.model, sizeof(out.model), "%s%s%s", vn, (vn[0] ? " " : ""), pr);
  smartmon::format_char_array(out.firmware, sizeof(out.firmware), inq + 32, 4);
  return true;
}

static bool identify(std::unique_ptr<smartmon::smart_device> & dev, smartmon_disk & out)
{
  if (!smartmon::smart_device::autodetect_open(dev))
    return false;

  // autodetect_open() may have replaced the device, e.g. a USB bridge with an
  // NVMe drive behind it (/dev/sdX -> sntasmedia), so read names afterwards
  copy_str(out.name, sizeof(out.name), dev->get_dev_name());
  copy_str(out.dev_type, sizeof(out.dev_type), dev->get_dev_type());

  bool ok = false;
  if (dev->is_nvme())
    ok = identify(dev->to_nvme(), out);
  else if (dev->is_ata())
    ok = identify(dev->to_ata(), out);
  else if (dev->is_scsi())
    ok = identify(dev->to_scsi(), out);
  dev->close();
  return ok;
}

/// Scan all disks and fill up to `max` entries of `out`.
/// Returns the number of entries filled, or -1 on error.
extern "C" int smartmon_scan(smartmon_disk * out, int max)
{
  try {
    static const bool initialized = [] {
      smartmon::smart_interface::init();
      // Needed to detect USB bridges by their USB ID, like smartctl does.
      // Without it, NVMe drives behind USB bridges aren't found.
      return smartmon::init_drive_database(true);
    }();
    if (!initialized)
      return -1;

    smartmon::smart_device_list devs{};
    if (!smartmon::smi()->scan_smart_devices(devs, smartmon::smart_devtype_list{}))
      return -1;

    int cnt = 0;
    for (unsigned i = 0; i < devs.size() && cnt < max; i++) {
      std::unique_ptr<smartmon::smart_device> dev(devs.release(i));
      smartmon_disk disk{};
      if (identify(dev, disk))
        out[cnt++] = disk;
    }
    return cnt;
  }
  catch (std::exception &) {
    return -1;
  }
}
