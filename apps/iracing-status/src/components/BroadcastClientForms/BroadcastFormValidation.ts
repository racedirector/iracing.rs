import { z } from "zod";

const uint32Max = 4_294_967_295;
const int32Min = -2_147_483_648;
const int32Max = 2_147_483_647;

export const requiredStringSchema = z
  .string()
  .trim()
  .min(1, "This field is required.");

export const optionalUint32Schema = z
  .string()
  .trim()
  .refine((value) => !value || /^\d+$/.test(value), {
    message: "Use digits only.",
  })
  .refine((value) => !value || Number(value) <= uint32Max, {
    message: `Use a value up to ${uint32Max}.`,
  });

export const requiredUint32Schema = optionalUint32Schema.refine(Boolean, {
  message: "This field is required.",
});

export const requiredInt32Schema = z
  .string()
  .trim()
  .min(1, "This field is required.")
  .refine((value) => /^-?\d+$/.test(value), {
    message: "Use an integer value.",
  })
  .refine(
    (value) => {
      const parsedValue = Number(value);
      return parsedValue >= int32Min && parsedValue <= int32Max;
    },
    { message: `Use a value from ${int32Min} to ${int32Max}.` },
  );

export const optionalFiniteNumberSchema = z
  .string()
  .trim()
  .refine((value) => !value || /^-?\d+(\.\d+)?$/.test(value), {
    message: "Use a numeric value.",
  })
  .refine((value) => !value || Number.isFinite(Number(value)), {
    message: "Use a finite numeric value.",
  });

export const requiredFiniteNumberSchema = optionalFiniteNumberSchema.refine(
  Boolean,
  { message: "This field is required." },
);
