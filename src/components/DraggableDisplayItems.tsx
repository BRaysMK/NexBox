import { Box, HStack, Text, Switch, useColorModeValue, Icon, useToast, Button } from "@chakra-ui/react";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";
import { GripVertical, Cpu, Thermometer, Activity, HardDrive, Key, Gauge, Fan, Zap, Clock, Download, Music4 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

export interface DisplayItem {
  id: string;
  label: string;
  enabled: boolean;
}

interface DraggableDisplayItemsProps {
  items: DisplayItem[];
  onReorder: (items: DisplayItem[]) => void;
  onToggle: (id: string, enabled: boolean) => void;
  disabledItems?: string[];
}

const ITEM_ICONS: Record<string, React.FC<{ size?: number }>> = {
  cpu_usage: Cpu,
  cpu_temp: Thermometer,
  cpu_clock: Clock,
  cpu_voltage: Zap,
  cpu_power: Zap,
  gpu_temp: Thermometer,
  gpu_usage: Activity,
  gpu_fan_speed: Fan,
  gpu_power: Zap,
  gpu_clock: Clock,
  gpu_memory_clock: Clock,
  gpu_voltage: Zap,
  gpu_vram: HardDrive,
  memory_usage: HardDrive,
  ssd_temp: HardDrive,
  delta_password: Key,
  game_ping: Gauge,
  fps: Activity,
  netease_lyric: Music4,
};

function SortableItem({
  item,
  onToggle,
  enabledCount,
  disabled,
}: {
  item: DisplayItem;
  onToggle: (id: string, enabled: boolean) => void;
  enabledCount: number;
  disabled?: boolean;
}) {
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const iconColor = useColorModeValue("gray.500", "#999999");
  const hoverBg = useColorModeValue("gray.50", "#1a1a1a");
  const dragBg = useColorModeValue("gray.100", "#222222");
  const toast = useToast();
  const { getActiveColor } = useThemeColor();

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: item.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.8 : 1,
    zIndex: isDragging ? 10 : 1,
  };

  const IconComponent = ITEM_ICONS[item.id] || Activity;

  const handleToggle = (checked: boolean) => {
    if (!checked && enabledCount <= 1) {
      toast({
        title: "至少需要保留一个显示项",
        status: "warning",
        duration: 2000,
        isClosable: true,
      });
      return;
    }
    onToggle(item.id, checked);
  };

  return (
    <HStack
      ref={setNodeRef}
      style={style}
      py={2}
      px={3}
      borderRadius="lg"
      bg={isDragging ? dragBg : "transparent"}
      _hover={{ bg: hoverBg }}
      transition="background 0.15s"
      spacing={3}
    >
      <Box
        cursor="grab"
        color={iconColor}
        display="flex"
        alignItems="center"
        {...attributes}
        {...listeners}
      >
        <GripVertical size={16} />
      </Box>
      <Icon as={() => <IconComponent size={18} />} color={item.enabled ? getActiveColor() : "gray.400"} />
      <Text color={textColor} fontSize="sm" flex={1}>
        {item.label}
      </Text>
      {item.id === "cpu_temp" && (
        <Button
          size="xs"
          variant="outline"
          color={getActiveColor()}
          borderColor={getActiveColor()}
          _hover={{ bg: hexToRgba(getActiveColor(), 0.1) }}
          leftIcon={<Download size={12} />}
          onClick={async () => {
            try {
              await invoke("run_pawnio_setup");
              toast({
                title: "安装程序已启动",
                status: "success",
                duration: 3000,
                isClosable: true,
              });
            } catch (e) {
              toast({
                title: typeof e === "string" ? e : "启动失败",
                status: "error",
                duration: 3000,
                isClosable: true,
              });
            }
          }}
          mr={1}
        >
          安装驱动
        </Button>
      )}
      <Switch
        isChecked={item.enabled}
        onChange={(e) => handleToggle(e.target.checked)}
        size="sm"
        isDisabled={disabled}
        sx={{
          '& .chakra-switch__track[data-checked]': {
            bg: getActiveColor(),
          },
        }}
      />
    </HStack>
  );
}

export function DraggableDisplayItems({
  items,
  onReorder,
  onToggle,
  disabledItems = [],
}: DraggableDisplayItemsProps) {
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  const enabledCount = items.filter((item) => item.enabled).length;

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;

    if (over && active.id !== over.id) {
      const oldIndex = items.findIndex((item) => item.id === active.id);
      const newIndex = items.findIndex((item) => item.id === over.id);

      onReorder(arrayMove(items, oldIndex, newIndex));
    }
  };

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragEnd={handleDragEnd}
    >
      <SortableContext
        items={items.map((item) => item.id)}
        strategy={verticalListSortingStrategy}
      >
        <Box>
          {items.map((item) => (
            <SortableItem
              key={item.id}
              item={item}
              onToggle={onToggle}
              enabledCount={enabledCount}
              disabled={disabledItems.includes(item.id)}
            />
          ))}
        </Box>
      </SortableContext>
    </DndContext>
  );
}
