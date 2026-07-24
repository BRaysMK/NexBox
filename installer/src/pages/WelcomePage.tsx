import { motion } from "framer-motion";

export default function WelcomePage() {
  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        flex: 1,
        gap: 32,
        padding: "32px 24px",
      }}
    >
      <motion.div
        initial={{ scale: 0.85, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 180, delay: 0.1 }}
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          gap: 16,
        }}
      >
        <img
          src="/logo/NexBoxW.png"
          alt="NexBox"
          style={{ width: 85, height: 85, objectFit: "contain" }}
        />
        <img
          src="/logo/Chinesew.png"
          alt="NexBox"
          style={{ height: 55, objectFit: "contain" }}
        />
      </motion.div>
    </motion.div>
  );
}
