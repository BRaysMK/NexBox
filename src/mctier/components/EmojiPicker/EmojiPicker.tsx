/**
 * Emoji表情选择器组件
 */

import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Box,
  Button,
  IconButton,
  HStack,
  SimpleGrid,
  useColorModeValue,
} from '@chakra-ui/react';
import './EmojiPicker.css';

interface EmojiPickerProps {
  onSelect: (emoji: string) => void;
  onClose: () => void;
}

// 常用Emoji分类
const EMOJI_CATEGORIES = {
  '笑脸': ['😀', '😃', '😄', '😁', '😆', '😅', '🤣', '😂', '🙂', '🙃', '😉', '😊', '😇', '🥰', '😍', '🤩', '😘', '😗', '😚', '😙'],
  '手势': ['👍', '👎', '👌', '✌️', '🤞', '🤟', '🤘', '🤙', '👈', '👉', '👆', '👇', '☝️', '✋', '🤚', '🖐️', '🖖', '👋', '🤝', '🙏'],
  '表情': ['🥺', '😢', '😭', '😤', '😠', '😡', '🤬', '😱', '😨', '😰', '😥', '😓', '🤗', '🤔', '🤭', '🤫', '🤥', '😶', '😐', '😑'],
  '符号': ['❤️', '💔', '💕', '💖', '💗', '💙', '💚', '💛', '🧡', '💜', '🖤', '💯', '💢', '💥', '💫', '💦', '💨', '🕳️', '💬', '👁️'],
  '其他': ['🎮', '🎯', '🎲', '🎰', '🎳', '🎉', '🎊', '🎈', '🎁', '🏆', '🥇', '🥈', '🥉', '⚽', '🏀', '🏈', '⚾', '🎾', '🏐', '🏓'],
};

export const EmojiPicker: React.FC<EmojiPickerProps> = ({ onSelect, onClose }) => {
  const [activeCategory, setActiveCategory] = useState<string>('笑脸');

  // 主题模式颜色
  const overlayBg = useColorModeValue('rgba(0, 0, 0, 0.3)', 'rgba(0, 0, 0, 0.5)');
  const panelBg = useColorModeValue('white', 'linear-gradient(145deg, rgba(30, 30, 40, 0.98) 0%, rgba(20, 20, 30, 0.98) 100%)');
  const panelShadow = useColorModeValue(
    '0 20px 60px rgba(0, 0, 0, 0.15), 0 0 0 1px #e2e8f0',
    '0 20px 60px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.1), inset 0 1px 0 rgba(255, 255, 255, 0.05)'
  );
  const categoryBg = useColorModeValue('gray.50', 'rgba(255, 255, 255, 0.05)');
  const categoryActiveBg = useColorModeValue('rgba(82, 196, 26, 0.1)', 'rgba(82, 196, 26, 0.15)');
  const categoryColor = useColorModeValue('gray.600', 'whiteAlpha.600');
  const categoryActiveBorder = useColorModeValue('1px solid rgba(82, 196, 26, 0.2)', '1px solid rgba(82, 196, 26, 0.3)');
  const categoryHoverBg = useColorModeValue('gray.100', 'rgba(255, 255, 255, 0.08)');
  const categoryHoverColor = useColorModeValue('gray.700', 'whiteAlpha.800');
  const closeBtnBg = useColorModeValue('gray.100', 'rgba(255, 255, 255, 0.05)');
  const closeBtnColor = useColorModeValue('gray.600', 'whiteAlpha.600');
  const closeBtnHoverBg = useColorModeValue('gray.200', 'rgba(255, 255, 255, 0.1)');
  const closeBtnHoverColor = useColorModeValue('gray.800', 'whiteAlpha.900');
  const emojiItemBg = useColorModeValue('gray.50', 'rgba(255, 255, 255, 0.03)');
  const emojiItemHoverBg = useColorModeValue('gray.100', 'rgba(255, 255, 255, 0.08)');

  const handleEmojiClick = (emoji: string) => {
    onSelect(emoji);
    onClose();
  };

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={onClose}
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: overlayBg,
        backdropFilter: 'blur(4px)',
        WebkitBackdropFilter: 'blur(4px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 10000,
      }}
    >
      <motion.div
        initial={{ scale: 0.8, opacity: 0, y: 20 }}
        animate={{ scale: 1, opacity: 1, y: 0 }}
        exit={{ scale: 0.8, opacity: 0, y: 20 }}
        onClick={(e) => e.stopPropagation()}
      >
        <Box
          bg={panelBg}
          borderRadius="16px"
          p={4}
          w="400px"
          maxW="90vw"
          maxH="500px"
          boxShadow={panelShadow}
          display="flex"
          flexDirection="column"
        >
          <HStack mb={3} gap={3} alignItems="center" justifyContent="space-between">
            <HStack gap={1} flex={1} overflowX="auto" className="emoji-categories-scroll">
              {Object.keys(EMOJI_CATEGORIES).map((category) => (
                <Button
                  key={category}
                  size="xs"
                  variant="ghost"
                  py="6px"
                  px="12px"
                  borderRadius="8px"
                  bg={activeCategory === category ? categoryActiveBg : categoryBg}
                  color={activeCategory === category ? '#52c41a' : categoryColor}
                  border={activeCategory === category ? categoryActiveBorder : '1px solid transparent'}
                  fontSize="12px"
                  fontWeight={500}
                  whiteSpace="nowrap"
                  flexShrink={0}
                  _hover={{ bg: categoryHoverBg, color: categoryHoverColor }}
                  onClick={() => setActiveCategory(category)}
                >
                  {category}
                </Button>
              ))}
            </HStack>
            <IconButton
              aria-label="关闭"
              size="sm"
              variant="ghost"
              w="32px"
              h="32px"
              borderRadius="8px"
              bg={closeBtnBg}
              color={closeBtnColor}
              onClick={onClose}
              _hover={{ bg: closeBtnHoverBg, color: closeBtnHoverColor }}
              icon={
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M18 6L6 18M6 6l12 12" />
                </svg>
              }
            />
          </HStack>
          <Box flex={1} overflowY="auto" overflowX="hidden" className="emoji-grid-scroll">
            <AnimatePresence mode="wait">
              <motion.div
                key={activeCategory}
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 20 }}
                transition={{ duration: 0.2 }}
              >
                <SimpleGrid
                  columns={{ base: 5, sm: 6, md: 7 }}
                  spacing={1}
                  p={1}
                >
                  {EMOJI_CATEGORIES[activeCategory as keyof typeof EMOJI_CATEGORIES].map((emoji, index) => (
                    <motion.button
                      key={`${emoji}-${index}`}
                      onClick={() => handleEmojiClick(emoji)}
                      whileHover={{ scale: 1.2 }}
                      whileTap={{ scale: 0.9 }}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        aspectRatio: '1',
                        borderRadius: '8px',
                        background: emojiItemBg,
                        border: 'none',
                        fontSize: '24px',
                        cursor: 'pointer',
                        transition: 'all 0.2s ease',
                        width: '100%',
                      }}
                      onMouseEnter={(e) => {
                        (e.currentTarget as HTMLElement).style.background = emojiItemHoverBg;
                      }}
                      onMouseLeave={(e) => {
                        (e.currentTarget as HTMLElement).style.background = emojiItemBg;
                      }}
                    >
                      {emoji}
                    </motion.button>
                  ))}
                </SimpleGrid>
              </motion.div>
            </AnimatePresence>
          </Box>
        </Box>
      </motion.div>
    </motion.div>
  );
};
