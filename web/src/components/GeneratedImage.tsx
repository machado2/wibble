import { useEffect, useRef, useState } from "react";
import styles from "@/styles/GeneratedImage.module.css";
import { BoxedImage } from "./BoxedImage";

export type GeneratedImageProps = {
    prompt: string;
    alt?: string;
    position?: string;
    cssClass?: string;
}

const cssClassFromPosition = (position: string | undefined) => {
    if (position === 'left') {
        return styles.imageleft;
    } else if (position === 'right') {
        return styles.imageright;
    } else {
        return styles.imagecenter;
    }
}

export const GeneratedImage = (props: GeneratedImageProps) => {
    const { prompt, alt, position } = props;
    const altText = alt || "";
    const imgWrapperRef = useRef<HTMLDivElement>(null);
    const [className, setClassName] = useState<string>(props.cssClass || cssClassFromPosition(position));

    useEffect(() => {
        const isGeneratedImageWrapper = (el: Element | null | undefined) =>
            el && el.classList.contains('generated-image-wrapper');

        const current = imgWrapperRef.current as HTMLDivElement;
        if (!current) {
            return;
        }

        let count: number = 0;
        for (let element: Element | null | undefined = current; isGeneratedImageWrapper(element); element = element?.previousElementSibling) {
            count++;
        }

        const order = (count - 1) % 3;
        if (order == 0 && !isGeneratedImageWrapper(current?.nextElementSibling)) {
            return;
        }

        const newCssClass = [styles.imageleft, styles.imageright, styles.imagebetween][order];
        if (className !== newCssClass) {
            setClassName(newCssClass);
        }

    }, []);

    return (
        <span className={`generated-image-wrapper`} ref={imgWrapperRef}>
            <BoxedImage alt={altText} className={className} prompt={prompt} />
        </span>
    );
}
