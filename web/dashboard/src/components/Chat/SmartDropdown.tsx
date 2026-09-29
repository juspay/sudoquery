import { useState, useRef, useEffect, useCallback } from 'react';
import { Box } from '@mui/material';
import { fontFamilyBody, colorBlue, colorInk, colorInk60 } from '../../theme/tokens';
import { ChevronDown } from 'lucide-react';

interface Option {
  value: string;
  label: string;
  subtext?: string;
}

interface SmartDropdownProps {
  value: string;
  options: (string | Option)[];
  onChange: (value: string) => void;
  buttonStyles?: React.CSSProperties;
}

export function SmartDropdown({ value, options, onChange, buttonStyles }: SmartDropdownProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [position, setPosition] = useState<'down' | 'up'>('down');
  const triggerRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const calculatePosition = useCallback(() => {
    if (!triggerRef.current) return;

    const rect = triggerRef.current.getBoundingClientRect();
    const dropdownHeight = 120; // Approximate max height
    const spaceBelow = window.innerHeight - rect.bottom;
    const spaceAbove = rect.top;

    if (spaceBelow < dropdownHeight && spaceAbove > dropdownHeight) {
      setPosition('up');
    } else {
      setPosition('down');
    }
  }, []);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node) &&
        triggerRef.current &&
        !triggerRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => {
    if (isOpen) {
      calculatePosition();
    }
  }, [isOpen, calculatePosition]);

  const handleSelect = (option: string) => {
    onChange(option);
    setIsOpen(false);
  };

  return (
    <Box sx={{ position: 'relative' }} ref={dropdownRef}>
      <Box
        ref={triggerRef}
        component="div"
        onClick={() => setIsOpen(!isOpen)}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          width: 'auto',
          height: '32px',
          padding: '4px 8px 4px 0',
          fontSize: '13px',
          fontFamily: fontFamilyBody,
          color: colorInk60,
          backgroundColor: 'transparent',
          cursor: 'pointer',
          outline: 'none',
          transition: 'all 0.15s',
          ...buttonStyles,
        }}
      >
        <span style={{ color: buttonStyles?.color }}>
          {(() => {
            const selectedOption = options.find(o => typeof o === 'object' ? o.value === value : o === value);
            if (typeof selectedOption === 'object') {
              return selectedOption.label;
            }
            return selectedOption || value;
          })()}
        </span>
        <ChevronDown size={16} color={buttonStyles?.color}/>
      </Box>

      {isOpen && (
        <Box
          sx={{
            position: 'absolute',
            left: 0,
            minWidth: '250px',
            maxWidth: '320px',
            backgroundColor: '#ffffff',
            borderRadius: '12px',
            boxShadow: '0 4px 20px rgba(0, 0, 0, 0.15)',
            zIndex: 1000,
            overflow: 'hidden',
            ...(position === 'down'
              ? { top: 'calc(100% + 4px)' }
              : { bottom: 'calc(100% + 4px)' }
            ),
          }}
        >
          {options.map((option, index) => {
            const optionValue = typeof option === 'object' ? option.value : option;
            const optionLabel = typeof option === 'object' ? option.label : option;
            const optionSubtext = typeof option === 'object' ? option.subtext : undefined;
            return (
              <Box
                key={optionValue}
                onClick={() => handleSelect(optionValue)}
                sx={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '12px 16px',
                  fontSize: '14px',
                  fontFamily: fontFamilyBody,
                  color: '#18160f',
                  cursor: 'pointer',
                  backgroundColor: value === optionValue ? '#f9fafb' : 'transparent',
                  borderBottom: index < options.length - 1 ? '1px solid #f3f4f6' : 'none',
                  '&:hover': {
                    backgroundColor: '#f3f4f6',
                  },
                }}
              >
                <Box>
                  <Box sx={{ fontWeight: 500 }}>{optionLabel}</Box>
                  {optionSubtext && (
                    <Box sx={{ fontSize: '12px', color: '#6b7280', marginTop: '4px' }}>
                      {optionSubtext}
                    </Box>
                  )}
                </Box>
                {value === optionValue && (
                  <svg
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke={colorBlue}
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    style={{ flexShrink: 0, marginLeft: '8px' }}
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                )}
              </Box>
            );
          })}
        </Box>
      )}
    </Box>
  );
}
