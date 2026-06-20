using System.Reflection;

var asm = Assembly.LoadFile(@"d:\NexBox\LibreHardwareMonitorLib.dll");

// Check SensorEventHandler
foreach (var t in asm.GetExportedTypes())
{
    if (t.Name.Contains("SensorEvent"))
    {
        Console.WriteLine($"=== {t.FullName} ===");
        Console.WriteLine($"  IsClass: {t.IsClass}, IsDelegate: {t.IsSubclassOf(typeof(Delegate))}");
        foreach (var m in t.GetMethods())
            Console.WriteLine($"  Method: {m.Name}, ReturnType: {m.ReturnType.Name}");
        foreach (var c in t.GetConstructors())
        {
            Console.WriteLine($"  Constructor params:");
            foreach (var p in c.GetParameters())
                Console.WriteLine($"    {p.ParameterType.Name} {p.Name}");
        }
    }
    if (t.Name == "SensorVisitor")
    {
        Console.WriteLine($"\n=== SensorVisitor ===");
        foreach (var c in t.GetConstructors())
        {
            Console.WriteLine($"  Constructor params:");
            foreach (var p in c.GetParameters())
                Console.WriteLine($"    {p.ParameterType.Name} {p.Name}");
        }
    }
}