import { useState, useLayoutEffect, RefObject } from 'react';

export function useContainerSize(ref: RefObject<HTMLElement>) {
  const [size, setSize] = useState({ width: 0, height: 0 });

  useLayoutEffect(() => {
    if (!ref.current) return;
    const updateSize = () => {
      const rect = ref.current.getBoundingClientRect();
      setSize({ width: rect.width, height: rect.height });
    };
    updateSize();
    const observer = new ResizeObserver(updateSize);
    observer.observe(ref.current);
    return () => observer.disconnect();
  }, [ref]);

  return size;
}