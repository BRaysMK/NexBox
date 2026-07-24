using System.Text.Json;
using LibreHardwareMonitor.Hardware;

namespace NexBoxMonitor;

class Program
{
    private static Computer? _computer;
    private static readonly SensorVisitor _visitor = new(_ => { });

    static int Main(string[] args)
    {
        try
        {
            _computer = new Computer
            {
                IsCpuEnabled = true,
                IsGpuEnabled = true,
                IsMemoryEnabled = true,
                IsMotherboardEnabled = true,
                IsStorageEnabled = true,
                IsNetworkEnabled = false,
                IsControllerEnabled = false,
                IsPsuEnabled = false,
            };

            _computer.Open();
            _computer.Accept(_visitor);

            var input = Console.In;
            var output = Console.Out;

            while (true)
            {
                var line = input.ReadLine();
                if (line == null)
                    break; // parent closed stdin

                line = line.Trim();
                if (line.Length == 0)
                    continue;

                try
                {
                    var cmd = JsonSerializer.Deserialize<ReadCommand>(line);
                    if (cmd?.Cmd == "read")
                    {
                        UpdateHardware(_computer.Hardware);
                        _computer.Accept(_visitor);

                        var sensors = new List<SensorReading>();
                        CollectSensors(_computer.Hardware, sensors);

                        var response = new SensorsResponse
                        {
                            UpdatedAt = DateTime.UtcNow.ToString("o"),
                            Sensors = sensors
                        };

                        var json = JsonSerializer.Serialize(response);
                        output.WriteLine(json);
                        output.Flush();
                    }
                    else if (cmd?.Cmd == "exit")
                    {
                        break;
                    }
                }
                catch (JsonException)
                {
                    // ignore malformed input
                }
            }
        }
        catch (Exception ex)
        {
            // Write error as JSON so parent can parse it
            var error = JsonSerializer.Serialize(new { error = ex.Message });
            Console.WriteLine(error);
            Console.Out.Flush();
            return 1;
        }
        finally
        {
            _computer?.Close();
        }

        return 0;
    }

    static void CollectSensors(IEnumerable<IHardware> hardwareList, List<SensorReading> sensors)
    {
        foreach (var hw in hardwareList)
        {
            // Recurse into sub-hardware (e.g. CPU cores, GPU fans)
            if (hw.SubHardware.Length > 0)
            {
                CollectSensors(hw.SubHardware, sensors);
            }

            foreach (var sensor in hw.Sensors)
            {
                if (!sensor.Value.HasValue)
                    continue;

                var reading = new SensorReading
                {
                    Hardware = hw.Name,
                    HardwareType = hw.HardwareType.ToString(),
                    SubHardware = hw.SubHardware.Length > 0 ? hw.Name : null,
                    Name = sensor.Name,
                    SensorType = sensor.SensorType.ToString(),
                    Value = sensor.Value.Value,
                    Unit = GetUnit(sensor.SensorType)
                };

                sensors.Add(reading);
            }
        }
    }

    static void UpdateHardware(IEnumerable<IHardware> hardwareList)
    {
        foreach (var hw in hardwareList)
        {
            hw.Update();
            if (hw.SubHardware.Length > 0)
                UpdateHardware(hw.SubHardware);
        }
    }

    static string? GetUnit(SensorType type)
    {
        return type switch
        {
            SensorType.Voltage => "V",
            SensorType.Clock => "MHz",
            SensorType.Temperature => "°C",
            SensorType.Load => "%",
            SensorType.Fan => "RPM",
            SensorType.Flow => "L/h",
            SensorType.Control => "%",
            SensorType.Level => "%",
            SensorType.Power => "W",
            SensorType.Data => "GB",
            SensorType.SmallData => "MB",
            SensorType.Factor => "",
            SensorType.Frequency => "Hz",
            SensorType.Throughput => "B/s",
            SensorType.Current => "A",
            SensorType.Energy => "mWh",
            SensorType.Noise => "dBA",
            SensorType.Humidity => "%",
            SensorType.TimeSpan => "s",
            _ => null
        };
    }
}