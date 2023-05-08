import type { Exograph } from './exograph.d.ts';

import type { ICaptchaValidatorContext } from './contexts.d.ts';

interface CaptchaChallenge {
    uuid: string,
    challenge: string,
}

export async function getChallenge(exograph: Exograph): Promise<CaptchaChallenge> {
    // generate a random, 5-letter string
    const challenge: string = (Math.random() + 1).toString(36).substring(7);

    // compute the proper response to the "CAPTCHA" by reversing `question`
    const properResponse: string = challenge.split("").reverse().join("");

    // store the record into the database
    const result = await exograph.executeQuery(`
        mutation ($properResponse: String!) {
            record: createCaptchaChallengeRecord(data: {
                properResponse: $properResponse
            }) {
                uuid
            }
        } 
    `, {
        "properResponse": properResponse
    });

    // return the CAPTCHA ID and challenge
    return {
        uuid: result.record.uuid,
        challenge: challenge
    }
}

export async function verifyCaptcha(exograph: Exograph, context: ICaptchaValidatorContext): Promise<boolean> {
    // get & delete CAPTCHA record -- we don't want to let users reuse challenge answers.
    const recordQuery = await exograph.executeQuery(`
        mutation ($uuid: String!) {
            record: deleteCaptchaChallengeRecord(uuid: $uuid) {
                properResponse
            }
        }
    `, {
        "uuid": context.uuid
    });

    // return whether the CAPTCHA matches the proper response
    return (context.response == recordQuery.record.properResponse)
}