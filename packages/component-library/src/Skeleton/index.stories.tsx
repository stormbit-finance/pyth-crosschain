import type { Meta, StoryObj } from "@storybook/react";

import { Skeleton as SkeletonComponent } from "./index.js";

const meta = {
  component: SkeletonComponent,
  argTypes: {
    label: {
      control: "text",
      table: {
        category: "Skeleton",
      },
    },
    width: {
      control: "number",
      table: {
        category: "Skeleton",
      },
    },
    round: {
      control: "boolean",
      table: {
        category: "Skeleton",
      },
    },
  },
} satisfies Meta<typeof SkeletonComponent>;
export default meta;

export const Skeleton = {
  args: {
    label: "Loading",
    width: 20,
    round: false,
  },
} satisfies StoryObj<typeof SkeletonComponent>;
