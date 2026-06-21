import {
  Box,
  Button,
  Heading,
  Text,
  VStack,
  useColorModeValue,
  useToast,
  HStack,
  IconButton,
  Card,
  CardBody,
  Slider,
  SliderTrack,
  SliderFilledTrack,
  SliderThumb,
  Grid,
  GridItem,
  Switch,
  FormControl,
  FormLabel,
  Alert,
  AlertIcon,
  AlertTitle,
  AlertDescription,
  AlertDialog,
  AlertDialogBody,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogContent,
  AlertDialogOverlay,
  useDisclosure,
} from "@chakra-ui/react";
import { CustomSelect } from "@/components/special/custom-select";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { AnimatedPage } from "@/components/ui/animated-page";
import { useNavigate } from "react-router-dom";
import { ArrowLeft, SlidersHorizontal, Download, Trash2 } from "lucide-react";
import { useThemeColor } from "@/contexts/theme-color-context";

interface EqBand {
  frequency: number;
  gain: number;
}

interface AudioDevice {
  id: string;
  name: string;
  is_default: boolean;
}

const DEFAULT_BANDS = [
  { frequency: 31, gain: 0 },
  { frequency: 62, gain: 0 },
  { frequency: 125, gain: 0 },
  { frequency: 250, gain: 0 },
  { frequency: 500, gain: 0 },
  { frequency: 1000, gain: 0 },
  { frequency: 2000, gain: 0 },
  { frequency: 4000, gain: 0 },
  { frequency: 8000, gain: 0 },
  { frequency: 16000, gain: 0 },
];

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

export default function EqTuningPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const toast = useToast();
  const navigate = useNavigate();

  const { getActiveColor, getContrastTextColor } = useThemeColor();
  const primaryColor = getActiveColor();
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const textColor = useColorModeValue("gray.600", "#a0a0a0");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const sliderBg = useColorModeValue("gray.100", "#1a1a1a");

  const [isEnabled, setIsEnabled] = useState(false);
  const [bands, setBands] = useState<EqBand[]>(DEFAULT_BANDS);
  const [masterGain, setMasterGain] = useState(0);
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string>("");
  const [isVirtualDriverInstalled, setIsVirtualDriverInstalled] = useState(true);
  const [loading, setLoading] = useState(true);
  const [isInstallingDriver, setIsInstallingDriver] = useState(false);
  const [isRemovingDriver, setIsRemovingDriver] = useState(false);
  const [isTogglingPower, setIsTogglingPower] = useState(false);
  const { isOpen: isRemoveOpen, onOpen: onRemoveOpen, onClose: onRemoveClose } = useDisclosure();
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    checkDriverAndLoadDevices();
  }, []);

  const checkDriverAndLoadDevices = async () => {
    try {
      setLoading(true);
      const installed = await invoke<boolean>("check_virtual_audio_driver");
      setIsVirtualDriverInstalled(installed);
      
      const deviceList = await invoke<AudioDevice[]>("get_audio_output_devices");
      setDevices(deviceList);
      
      const config = await invoke<any>("get_eq_config");
      setIsEnabled(config.enabled);
      setBands(config.bands);
      setMasterGain(config.master_gain);
      
      if (config.output_device_id) {
        setSelectedDevice(config.output_device_id);
      } else {
        const defaultDev = deviceList.find(d => d.is_default);
        if (defaultDev) setSelectedDevice(defaultDev.id);
      }
    } catch (error) {
      console.error("Failed to load audio data:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleBandChange = (index: number, value: number) => {
    const newBands = [...bands];
    newBands[index].gain = value;
    setBands(newBands);
    invoke("set_eq_band", { index, gain: value });
  };

  const handleTogglePower = async () => {
    if (!isVirtualDriverInstalled) {
      toast({
        title: t("eqTuning.virtualAudioNotFound"),
        description: t("eqTuning.driverNote"),
        status: "warning",
        duration: 3000,
      });
      return;
    }

    const newState = !isEnabled;
    try {
      setIsTogglingPower(true);
      setIsEnabled(newState);
      await invoke("set_eq_enabled", { enabled: newState });
    } catch (error) {
      setIsEnabled(!newState);
      toast({
        title: t("gpuRename.error"),
        description: String(error),
        status: "error",
        duration: 3000,
      });
    } finally {
      setIsTogglingPower(false);
    }
  };

  const handleReset = () => {
    setBands(DEFAULT_BANDS.map(b => ({ ...b })));
    setMasterGain(0);
    invoke("reset_eq");
  };

  const handleDeviceChange = (deviceId: string) => {
    setSelectedDevice(deviceId);
    invoke("set_output_device", { deviceId });
  };

  const handleMasterGainChange = (value: number) => {
    setMasterGain(value);
    invoke("set_master_gain", { gain: value });
  };

  const handleInstallDriver = async () => {
    try {
      setIsInstallingDriver(true);
      const message = await invoke<string>("install_eq_driver");

      let installed = false;
      for (let attempt = 0; attempt < 3; attempt += 1) {
        await sleep(1500);
        await checkDriverAndLoadDevices();
        const detected = await invoke<boolean>("check_virtual_audio_driver");
        if (detected) {
          installed = true;
          break;
        }
      }

      toast({
        title: t("eqTuning.installDriver"),
        description: installed
          ? "虚拟声卡驱动已安装完成，现可启用 EQ 调音"
          : message,
        status: installed ? "success" : "info",
        duration: 5000,
      });
    } catch (error) {
      toast({
        title: t("gpuRename.error"),
        description: String(error),
        status: "error",
        duration: 3000,
      });
    } finally {
      setIsInstallingDriver(false);
    }
  };

  const handleRemoveDriver = async () => {
    try {
      setIsRemovingDriver(true);
      onRemoveClose();
      const message = await invoke<string>("remove_eq_driver");

      setIsVirtualDriverInstalled(false);
      setIsEnabled(false);

      toast({
        title: t("eqTuning.removeDriver"),
        description: message,
        status: "success",
        duration: 5000,
      });
    } catch (error) {
      toast({
        title: t("gpuRename.error"),
        description: String(error),
        status: "error",
        duration: 5000,
      });
    } finally {
      setIsRemovingDriver(false);
    }
  };

  const formatFreq = (freq: number) => {
    return freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
  };

  const content = (
    <VStack align="start" spacing={6} w="full">
      <HStack w="full" justify="space-between">
        <HStack>
          <IconButton
            aria-label={t("eqTuning.back")}
            icon={<ArrowLeft size={20} />}
            variant="ghost"
            onClick={() => navigate("/builtin-tools")}
            color={headingColor}
          />
          <SlidersHorizontal size={28} color={headingColor} />
          <Heading size="lg" color={headingColor} fontWeight="700">
            {t("eqTuning.title")}
          </Heading>
        </HStack>

        <HStack spacing={4}>
          <FormControl display="flex" alignItems="center">
            <FormLabel htmlFor="eq-power" mb="0" color={textColor} fontSize="sm">
              {t("eqTuning.power")}
            </FormLabel>
            <Switch
              id="eq-power"
              isChecked={isEnabled}
              onChange={handleTogglePower}
              colorScheme="teal"
              isDisabled={!isVirtualDriverInstalled || loading || isTogglingPower}
            />
          </FormControl>
          <Button
            size="sm"
            variant="outline"
            onClick={handleReset}
            borderColor={cardBorder}
            color={textColor}
            _hover={{ bg: sliderBg }}
          >
            {t("eqTuning.reset")}
          </Button>
          {isVirtualDriverInstalled && (
            <IconButton
              aria-label={t("eqTuning.removeDriver")}
              icon={<Trash2 size={16} />}
              size="sm"
              variant="ghost"
              colorScheme="red"
              onClick={onRemoveOpen}
              isLoading={isRemovingDriver}
            />
          )}
        </HStack>
      </HStack>

      {!isVirtualDriverInstalled && (
        <Alert
          status="warning"
          variant="subtle"
          flexDirection="column"
          alignItems="center"
          justifyContent="center"
          textAlign="center"
          borderRadius="xl"
          py={6}
          bg={useColorModeValue("orange.50", "rgba(251, 211, 141, 0.1)")}
          border="1px solid"
          borderColor={useColorModeValue("orange.200", "rgba(251, 211, 141, 0.2)")}
        >
          <AlertIcon boxSize="40px" mr={0} />
          <AlertTitle mt={4} mb={1} fontSize="lg">
            {t("eqTuning.virtualAudioNotFound")}
          </AlertTitle>
          <AlertDescription maxWidth="sm" color={textColor}>
            {t("eqTuning.driverNote")}
          </AlertDescription>
          <Button
            mt={4}
            leftIcon={<Download size={16} />}
            colorScheme="orange"
            onClick={handleInstallDriver}
            isLoading={isInstallingDriver}
          >
            {t("eqTuning.installDriver")}
          </Button>
        </Alert>
      )}

      <VStack w="full" spacing={8} opacity={isEnabled ? 1 : 0.5} transition="opacity 0.2s">
        <Box w="full">
          <Text color={textColor} fontSize="sm" mb={4} fontWeight="600">
            {t("eqTuning.outputDevice")}
          </Text>
          <CustomSelect
            value={selectedDevice}
            onChange={handleDeviceChange}
            options={devices.map(d => ({ value: d.id, label: d.name }))}
            placeholder={t("eqTuning.outputDevice")}
            width="100%"
          />
        </Box>

        <Grid templateColumns="repeat(10, 1fr)" gap={2} w="full" h="300px">
          {bands.map((band, index) => (
            <GridItem key={band.frequency} h="full">
              <VStack h="full" spacing={2}>
                <Text fontSize="xs" color={textColor} fontWeight="bold">
                  {band.gain > 0 ? `+${band.gain}` : band.gain}
                </Text>
                <Box h="200px" py={2}>
                  <Slider
                    aria-label={`freq-${band.frequency}`}
                    orientation="vertical"
                    min={-12}
                    max={12}
                    step={1}
                    value={band.gain}
                    onChange={(v) => handleBandChange(index, v)}
                    isDisabled={!isEnabled}
                    h="full"
                  >
                    <SliderTrack bg={sliderBg} borderRadius="full">
                      <SliderFilledTrack bg={primaryColor} />
                    </SliderTrack>
                    <SliderThumb boxSize={4} bg="white" border="2px solid" borderColor={primaryColor} />
                  </Slider>
                </Box>
                <Text fontSize="xs" color={textColor} transform="rotate(-45deg)" mt={2}>
                  {formatFreq(band.frequency)}
                </Text>
              </VStack>
            </GridItem>
          ))}
        </Grid>

        <Box w="full" pt={4}>
          <HStack justify="space-between" mb={2}>
            <Text color={textColor} fontSize="sm" fontWeight="600">
              {t("eqTuning.masterGain")}
            </Text>
            <Text color={primaryColor} fontWeight="bold">
              {masterGain > 0 ? `+${masterGain}` : masterGain} dB
            </Text>
          </HStack>
          <Slider
            aria-label="master-gain"
            min={-12}
            max={12}
            step={1}
            value={masterGain}
            onChange={handleMasterGainChange}
            isDisabled={!isEnabled}
          >
            <SliderTrack bg={sliderBg} h="6px" borderRadius="full">
              <SliderFilledTrack bg={primaryColor} />
            </SliderTrack>
            <SliderThumb boxSize={5} bg="white" border="2px solid" borderColor={primaryColor} />
          </Slider>
        </Box>
      </VStack>

      <AlertDialog
        isOpen={isRemoveOpen}
        leastDestructiveRef={cancelRef}
        onClose={onRemoveClose}
      >
        <AlertDialogOverlay>
          <AlertDialogContent>
            <AlertDialogHeader fontSize="lg" fontWeight="bold">
              {t("eqTuning.removeDriver")}
            </AlertDialogHeader>

            <AlertDialogBody>
              {t("eqTuning.removeDriverConfirm")}
            </AlertDialogBody>

            <AlertDialogFooter>
              <Button ref={cancelRef} onClick={onRemoveClose}>
                {t("common.cancel")}
              </Button>
              <Button
                colorScheme="red"
                onClick={handleRemoveDriver}
                ml={3}
                isLoading={isRemovingDriver}
              >
                {t("common.confirm")}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>
    </VStack>
  );

  return (
    <AnimatedPage>
      <Box pt={8}>
        {liquidGlassEnabled ? (
          <LiquidGlassCard
            w="full"
            boxShadow="2xl"
            overflow="hidden"
            position="relative"
            p={6}
          >
            {content}
          </LiquidGlassCard>
        ) : (
          <Card
            bg={cardBg}
            borderColor={cardBorder}
            borderWidth="1px"
            w="full"
            boxShadow="2xl"
            overflow="hidden"
            position="relative"
          >
            <CardBody p={6}>
              {content}
            </CardBody>
          </Card>
        )}
      </Box>
    </AnimatedPage>
  );
}
