import { Box, HStack, Text, Switch, useColorModeValue, Icon, useToast } from "@chakra-ui/react";
import { useThemeColor } from "@/contexts/theme-color-context";
import { GripVertical, Cpu, Thermometer, Activity, HardDrive, Key, Gauge } from "lucide-react";
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
}

const ITEM_ICONS: Record<string, React.FC<{ size?: number }>> = {
  cpu_usage: Cpu,
  gpu_temp: Thermometer,
  gpu_usage: Activity,
  memory_usage: HardDrive,
  delta_password: Key,
  game_ping: Gauge,
  fps: Activity,
};

function SortableItem({
  item,
  onToggle,
  enabledCount,
}: {
  item: DisplayItem;
  onToggle: (id: string, enabled: boolean) => void;
  enabledCount: number;
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
      <Switch
        isChecked={item.enabled}
        onChange={(e) => handleToggle(e.target.checked)}
        size="sm"
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
            />
          ))}
        </Box>
      </SortableContext>
    </DndContext>
  );
}
